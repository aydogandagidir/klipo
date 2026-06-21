//! Tokio task that consumes `ClipboardEvent`s from the OS-specific watcher
//! and persists them via `Storage::insert_clip`.
//!
//! Order of operations (matters for the security non-negotiables):
//!   1. Excluded-app filter (drop silently — no logging of content).
//!   2. Sensitive-content scan (regex set; flags but does NOT drop).
//!   3. SHA-256 hash for dedup. Storage layer dedups via partial unique index
//!      and returns `Bumped` instead of `Inserted` on duplicates.
//!   4. For binary kinds (image): write blob to disk under `<db_dir>/blobs/`,
//!      schedule a thumbnail task (best-effort, non-blocking).
//!   5. Insert into `clips` table.
//!   6. Emit `clip:new` (or `clip:bumped`) Tauri event.

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::clipboard::classify;
use crate::clipboard::sensitive;
use crate::clipboard::{CapturedKind, ClipboardEvent};
use crate::storage::blob::{write_blob, write_thumbnail};
use crate::storage::clips::{ClipKind, InsertOutcome, NewClip};
use crate::storage::Storage;

pub async fn run(mut rx: UnboundedReceiver<ClipboardEvent>, storage: Storage, app: AppHandle) {
    tracing::info!(target: "klipo::pipeline", "clipboard pipeline online");

    while let Some(event) = rx.recv().await {
        process_one(&storage, &app, event).await;
    }

    tracing::info!(target: "klipo::pipeline", "clipboard pipeline ended");
}

async fn process_one(storage: &Storage, app: &AppHandle, event: ClipboardEvent) {
    // 1. Excluded app filter.
    if let Some(ref id) = event.source_app {
        match storage.is_app_excluded(id).await {
            Ok(true) => {
                tracing::debug!(
                    target: "klipo::pipeline",
                    source = %id,
                    "drop: source app is excluded"
                );
                return;
            }
            Ok(false) => {}
            Err(e) => tracing::warn!(
                target: "klipo::pipeline",
                error = %e,
                "excluded-app lookup failed; proceeding without filter"
            ),
        }
    }

    // 1b. License / trial gate. Honest-user deterrent (M8): if the user has
    //     no license AND the 14-day trial has expired, drop the event before
    //     we pay the cost of the sensitive-content scan or any disk I/O. The
    //     popup overlay tells the user what to do; the watcher keeps running
    //     so capture resumes the moment they activate or re-enter trial via
    //     a fresh install.
    if !crate::license::manager::capture_allowed(storage).await {
        tracing::debug!(
            target: "klipo::pipeline",
            "capture dropped — trial expired and no license"
        );
        return;
    }

    // 2. Sensitive scan (text-bearing payloads only).
    let sensitive_flag = match event.text.as_deref() {
        Some(text) => {
            let s = sensitive::scan(text);
            if s.is_sensitive() {
                tracing::info!(
                    target: "klipo::pipeline",
                    matched = ?s.matched,
                    "sensitive content detected"
                );
                true
            } else {
                false
            }
        }
        None => false,
    };

    // 2b. Content classification (text clips only). The classifier's key seeds
    //     the clip's first auto label *after* insert (see below) — distinct
    //     from the security `sensitive` flag. Only plain text is classified:
    //     file clips carry a JSON path list, and html/rtf already announce
    //     themselves via their kind.
    let auto_key: Option<String> = match event.kind {
        CapturedKind::Text => event
            .text
            .as_deref()
            .and_then(classify::classify)
            .map(str::to_string),
        _ => None,
    };

    // 3. Hash whichever payload exists.
    let content_hash = match (&event.text, &event.bytes) {
        (Some(t), _) => sha256_hex(t.as_bytes()),
        (None, Some(b)) => sha256_hex(b),
        (None, None) => {
            tracing::debug!(target: "klipo::pipeline", "drop: empty event");
            return;
        }
    };

    // 4. For binary kinds, write blob to disk before inserting the row so
    //    the DB row never points to a missing file.
    let blob_path: Option<String> = match (&event.kind, &event.bytes) {
        (CapturedKind::Image, Some(bytes)) => {
            let Some(blob_root) = storage.blob_root() else {
                tracing::warn!(
                    target: "klipo::pipeline",
                    "blob_root unavailable (in-memory storage?); skipping image"
                );
                return;
            };
            match write_blob(&blob_root, &content_hash, "png", bytes).await {
                Ok(rel) => {
                    // Best-effort thumbnail in background — never blocks insert.
                    let thumb_root = storage.thumb_root();
                    let bytes_clone = bytes.clone();
                    let hash_for_thumb = content_hash.clone();
                    if let Some(thumb_root) = thumb_root {
                        tokio::spawn(async move {
                            let _ =
                                write_thumbnail(&thumb_root, &hash_for_thumb, bytes_clone).await;
                        });
                    }
                    Some(rel)
                }
                Err(e) => {
                    tracing::warn!(
                        target: "klipo::pipeline",
                        error = %e,
                        "blob write failed; skipping image"
                    );
                    return;
                }
            }
        }
        _ => None,
    };

    let kind = match event.kind {
        CapturedKind::Text => ClipKind::Text,
        CapturedKind::Image => ClipKind::Image,
        CapturedKind::File => ClipKind::File,
        CapturedKind::Rtf => ClipKind::Rtf,
        CapturedKind::Html => ClipKind::Html,
    };

    // For binary kinds we don't store the bytes inline — text_content stays
    // None. For text-bearing kinds (text/html/rtf/file-as-json) we use the
    // text payload directly.
    let text_for_db = match event.kind {
        CapturedKind::Image => None,
        _ => event.text,
    };

    let new_clip = NewClip {
        kind,
        content_hash: content_hash.clone(),
        text_content: text_for_db,
        blob_path,
        size_bytes: event.size_bytes as i64,
        source_app: event.source_app,
        source_url: None,
        source_window_title: event.source_window_title,
        sensitive: sensitive_flag,
    };

    // 5. Insert.
    match storage.insert_clip(new_clip).await {
        Ok(InsertOutcome::Inserted { id }) => {
            tracing::info!(
                target: "klipo::pipeline",
                id = %id,
                hash_prefix = &content_hash[..12],
                kind = kind.as_str(),
                size = event.size_bytes,
                "captured new clip"
            );
            // Seed the auto-detected label (before emitting so the popup's
            // refresh sees it). Best-effort: a link failure must not lose the clip.
            if let Some(ref key) = auto_key {
                if let Err(e) = storage.link_auto_label(&id, key).await {
                    tracing::warn!(
                        target: "klipo::pipeline",
                        error = %e,
                        "auto-label link failed"
                    );
                }
            }
            let _ = app.emit("clip:new", id);
        }
        Ok(InsertOutcome::Bumped { id }) => {
            tracing::debug!(
                target: "klipo::pipeline",
                id = %id,
                "duplicate; bumped existing clip"
            );
            let _ = app.emit("clip:bumped", id);
        }
        Err(e) => tracing::warn!(
            target: "klipo::pipeline",
            error = %e,
            "insert failed"
        ),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_stable() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"hello world"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }
}
