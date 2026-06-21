//! Clip CRUD on top of the v0.1 schema.
//!
//! "Clip" is one user-facing item the clipboard manager has captured: a piece
//! of text, an image, a list of file paths, or a richer format (RTF/HTML).
//!
//! Insert semantics — important:
//!   - Hash-based dedup: a duplicate `content_hash` of an existing,
//!     non-deleted clip does NOT create a new row. Instead the existing row's
//!     `created_at` is bumped to "now" so it floats to the top of the list.
//!   - Excluded apps are NOT filtered here — that policy lives in the
//!     clipboard watcher (M3). This module is a pure data layer.
//!
//! Soft delete: `deleted_at` is set; row stays in DB until retention worker
//! garbage-collects it (M2 follow-up; see `docs/storage.md` §5).

use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sqlx::Row;

use super::error::{StorageError, StorageResult};
use super::Storage;

/// User-visible clip kind. Mirrors the SQL CHECK constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipKind {
    Text,
    Image,
    File,
    Rtf,
    Html,
}

impl ClipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ClipKind::Text => "text",
            ClipKind::Image => "image",
            ClipKind::File => "file",
            ClipKind::Rtf => "rtf",
            ClipKind::Html => "html",
        }
    }
}

impl FromStr for ClipKind {
    type Err = StorageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "text" => ClipKind::Text,
            "image" => ClipKind::Image,
            "file" => ClipKind::File,
            "rtf" => ClipKind::Rtf,
            "html" => ClipKind::Html,
            other => return Err(StorageError::InvalidKind(other.to_string())),
        })
    }
}

/// One label attached to a clip. `auto_key` is the stable classifier key for
/// auto-detected labels (drives the chip color and survives a rename); `None`
/// for user-created labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub name: String,
    pub auto_key: Option<String>,
}

/// A clip as stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: String,
    pub kind: ClipKind,
    pub content_hash: String,
    pub text_content: Option<String>,
    pub blob_path: Option<String>,
    pub size_bytes: i64,
    pub source_app: Option<String>,
    pub source_url: Option<String>,
    pub source_window_title: Option<String>,
    pub created_at: i64,
    pub pinned: bool,
    pub sensitive: bool,
    /// Optional user-given name. When set, the UI shows this instead of the
    /// first-line preview. Searchable (folded into the FTS index).
    pub title: Option<String>,
    /// Labels attached to this clip. The capture pipeline seeds one
    /// auto-detected label; the user can add/rename/remove more. Populated by a
    /// `json_group_array` subquery on the read path; empty when none.
    #[serde(default)]
    pub labels: Vec<Label>,
}

/// Input shape for `insert_clip`. The watcher (M3) builds these.
#[derive(Debug, Clone)]
pub struct NewClip {
    pub kind: ClipKind,
    pub content_hash: String,
    pub text_content: Option<String>,
    pub blob_path: Option<String>,
    pub size_bytes: i64,
    pub source_app: Option<String>,
    pub source_url: Option<String>,
    pub source_window_title: Option<String>,
    pub sensitive: bool,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn new_uuid_v7() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Outcome of `insert_clip` — caller can react accordingly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertOutcome {
    /// Clip inserted; `id` is the new clip's id.
    Inserted { id: String },
    /// Clip was a duplicate of `id`; that row's `created_at` was bumped.
    Bumped { id: String },
}

impl Storage {
    /// Insert a new clip. Hash-based dedup — see module docs.
    pub async fn insert_clip(&self, input: NewClip) -> StorageResult<InsertOutcome> {
        let now = now_ms();

        // Look up existing live (non-deleted) row with same hash.
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM clips WHERE content_hash = ? AND deleted_at IS NULL LIMIT 1",
        )
        .bind(&input.content_hash)
        .fetch_optional(self.pool())
        .await?;

        if let Some((existing_id,)) = existing {
            sqlx::query(
                "UPDATE clips SET created_at = ?, sync_version = sync_version + 1 WHERE id = ?",
            )
            .bind(now)
            .bind(&existing_id)
            .execute(self.pool())
            .await?;
            return Ok(InsertOutcome::Bumped { id: existing_id });
        }

        let id = new_uuid_v7();
        sqlx::query(
            "INSERT INTO clips (
                id, kind, content_hash, text_content, blob_path, size_bytes,
                source_app, source_url, source_window_title,
                created_at, sensitive
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(input.kind.as_str())
        .bind(&input.content_hash)
        .bind(&input.text_content)
        .bind(&input.blob_path)
        .bind(input.size_bytes)
        .bind(&input.source_app)
        .bind(&input.source_url)
        .bind(&input.source_window_title)
        .bind(now)
        .bind(input.sensitive as i64)
        .execute(self.pool())
        .await?;

        Ok(InsertOutcome::Inserted { id })
    }

    /// Get a single clip by id. Returns NotFound if missing or already deleted.
    pub async fn get_clip(&self, id: &str) -> StorageResult<Clip> {
        let row = sqlx::query(
            "SELECT id, kind, content_hash, text_content, blob_path, size_bytes,
                    source_app, source_url, source_window_title,
                    created_at, pinned, sensitive, title,
                    (SELECT json_group_array(json_object('name', cl.name, 'autoKey', cl.auto_key))
                       FROM clip_labels cl WHERE cl.clip_id = clips.id) AS labels
             FROM clips
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await?;

        match row {
            Some(row) => row_to_clip(&row),
            None => Err(StorageError::NotFound(id.to_string())),
        }
    }

    /// List recent non-deleted clips (pinned first, then by `created_at` desc).
    pub async fn list_clips(&self, limit: i64, offset: i64) -> StorageResult<Vec<Clip>> {
        let rows = sqlx::query(
            "SELECT id, kind, content_hash, text_content, blob_path, size_bytes,
                    source_app, source_url, source_window_title,
                    created_at, pinned, sensitive, title,
                    (SELECT json_group_array(json_object('name', cl.name, 'autoKey', cl.auto_key))
                       FROM clip_labels cl WHERE cl.clip_id = clips.id) AS labels
             FROM clips
             WHERE deleted_at IS NULL
             ORDER BY pinned DESC, created_at DESC
             LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool())
        .await?;

        rows.iter().map(row_to_clip).collect()
    }

    /// Set or unset the `pinned` flag.
    pub async fn pin_clip(&self, id: &str, pinned: bool) -> StorageResult<()> {
        let result = sqlx::query(
            "UPDATE clips SET pinned = ?, sync_version = sync_version + 1
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(pinned as i64)
        .bind(id)
        .execute(self.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// Soft-delete a clip (sets `deleted_at = now`).
    pub async fn soft_delete(&self, id: &str) -> StorageResult<()> {
        let now = now_ms();
        let result = sqlx::query(
            "UPDATE clips SET deleted_at = ?, sync_version = sync_version + 1
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(id)
        .execute(self.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// Set or clear a clip's user title. An empty / whitespace-only string
    /// clears it back to `NULL`. Bumps `sync_version` so the change propagates.
    /// The FTS `clips_au` trigger re-indexes the row, so a new title becomes
    /// searchable immediately.
    pub async fn set_clip_title(&self, id: &str, title: Option<&str>) -> StorageResult<()> {
        let normalized = title.map(str::trim).filter(|t| !t.is_empty());
        let result = sqlx::query(
            "UPDATE clips SET title = ?, sync_version = sync_version + 1
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(normalized)
        .bind(id)
        .execute(self.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// Link an auto-detected label (by classifier key) to a clip. Used by the
    /// capture pipeline and by `reclassify_all`. Uses the label's current
    /// display name if the user has renamed it (looked up by `auto_key`),
    /// otherwise the default from `classify::auto_label_name`. Idempotent.
    pub async fn link_auto_label(&self, clip_id: &str, key: &str) -> StorageResult<()> {
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT name FROM clip_labels WHERE auto_key = ? LIMIT 1")
                .bind(key)
                .fetch_optional(self.pool())
                .await?;
        let name = existing
            .map(|(n,)| n)
            .unwrap_or_else(|| crate::clipboard::classify::auto_label_name(key).to_string());

        sqlx::query(
            "INSERT OR IGNORE INTO clip_labels (clip_id, name, auto_key, added_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(clip_id)
        .bind(&name)
        .bind(key)
        .bind(now_ms())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Add a label to a clip. If the name already exists as an auto label
    /// elsewhere, the link inherits its `auto_key` (so it keeps the right
    /// color); otherwise it's a custom label (`auto_key = NULL`). Idempotent —
    /// re-adding the same name is a no-op. Returns the trimmed name stored.
    pub async fn add_label(&self, clip_id: &str, name: &str) -> StorageResult<String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StorageError::InvalidKind(
                "label name must not be empty".to_string(),
            ));
        }
        if name.chars().any(|c| c.is_control()) {
            return Err(StorageError::InvalidKind(
                "label name must not contain control characters".to_string(),
            ));
        }

        let exists: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM clips WHERE id = ? AND deleted_at IS NULL")
                .bind(clip_id)
                .fetch_optional(self.pool())
                .await?;
        if exists.is_none() {
            return Err(StorageError::NotFound(clip_id.to_string()));
        }

        let auto_key: Option<String> = sqlx::query_as::<_, (String,)>(
            "SELECT auto_key FROM clip_labels WHERE name = ? AND auto_key IS NOT NULL LIMIT 1",
        )
        .bind(name)
        .fetch_optional(self.pool())
        .await?
        .map(|(k,)| k);

        sqlx::query(
            "INSERT OR IGNORE INTO clip_labels (clip_id, name, auto_key, added_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(clip_id)
        .bind(name)
        .bind(&auto_key)
        .bind(now_ms())
        .execute(self.pool())
        .await?;

        sqlx::query("UPDATE clips SET sync_version = sync_version + 1 WHERE id = ?")
            .bind(clip_id)
            .execute(self.pool())
            .await?;

        Ok(name.to_string())
    }

    /// Remove a label from a clip. No-op if not present. Bumps `sync_version`
    /// only when a link was actually removed.
    pub async fn remove_label(&self, clip_id: &str, name: &str) -> StorageResult<()> {
        let name = name.trim();
        let result = sqlx::query("DELETE FROM clip_labels WHERE clip_id = ? AND name = ?")
            .bind(clip_id)
            .bind(name)
            .execute(self.pool())
            .await?;

        if result.rows_affected() > 0 {
            sqlx::query("UPDATE clips SET sync_version = sync_version + 1 WHERE id = ?")
                .bind(clip_id)
                .execute(self.pool())
                .await?;
        }
        Ok(())
    }

    /// Rename a label everywhere it occurs (global). `OR REPLACE` merges the
    /// edge case where a clip already carries both the old and the new name.
    /// Bumps `sync_version` for every affected clip.
    pub async fn rename_label(&self, old: &str, new: &str) -> StorageResult<()> {
        let new = new.trim();
        if new.is_empty() {
            return Err(StorageError::InvalidKind(
                "label name must not be empty".to_string(),
            ));
        }
        if new.chars().any(|c| c.is_control()) {
            return Err(StorageError::InvalidKind(
                "label name must not contain control characters".to_string(),
            ));
        }

        sqlx::query("UPDATE OR REPLACE clip_labels SET name = ? WHERE name = ?")
            .bind(new)
            .bind(old)
            .execute(self.pool())
            .await?;

        sqlx::query(
            "UPDATE clips SET sync_version = sync_version + 1
             WHERE id IN (SELECT clip_id FROM clip_labels WHERE name = ?)",
        )
        .bind(new)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// List the label vocabulary (names actually in use on live clips) with a
    /// representative `auto_key` and usage count, most-used first. Powers the
    /// popup filter chips + the editor's add-label autocomplete.
    pub async fn list_labels(&self) -> StorageResult<Vec<LabelInfo>> {
        let rows = sqlx::query(
            "SELECT cl.name AS name,
                    MAX(cl.auto_key) AS auto_key,
                    COUNT(c.id) AS count
             FROM clip_labels cl
             JOIN clips c ON c.id = cl.clip_id AND c.deleted_at IS NULL
             GROUP BY cl.name
             ORDER BY count DESC, cl.name ASC",
        )
        .fetch_all(self.pool())
        .await?;

        rows.iter()
            .map(|row| {
                Ok(LabelInfo {
                    name: row.try_get("name")?,
                    auto_key: row.try_get("auto_key")?,
                    count: row.try_get("count")?,
                })
            })
            .collect()
    }

    /// Count of live (non-deleted) clips. Useful for quick UI badges + tests.
    pub async fn count_live(&self) -> StorageResult<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM clips WHERE deleted_at IS NULL")
            .fetch_one(self.pool())
            .await?;
        Ok(row.0)
    }

    /// Whether the given source app identifier appears in the excluded list.
    ///
    /// Used by the watcher pipeline to drop clipboard captures from password
    /// managers and other sensitive apps. Comparison is case-sensitive and
    /// matches verbatim — we trust the seed list to be canonical (the
    /// migration uses each tool's vendor-published process / bundle name).
    pub async fn is_app_excluded(&self, identifier: &str) -> StorageResult<bool> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM excluded_apps WHERE bundle_id = ?")
            .bind(identifier)
            .fetch_one(self.pool())
            .await?;
        Ok(row.0 > 0)
    }

    /// Read one entry from the `settings` k/v table.
    ///
    /// Returns `None` if the key has never been set. The migration seeds a
    /// handful of well-known keys (theme, hotkey, history_limit, …) so the
    /// Settings UI can rely on them existing on a fresh install.
    pub async fn get_setting(&self, key: &str) -> StorageResult<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(self.pool())
            .await?;
        Ok(row.map(|(v,)| v))
    }

    /// Upsert one entry into the `settings` k/v table.
    ///
    /// Always overwrites the existing value — the Settings UI is the only
    /// caller for now, and it sends the full new value each time.
    pub async fn set_setting(&self, key: &str, value: &str) -> StorageResult<()> {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// List all excluded-app entries, sorted by `added_at DESC` so the most
    /// recently added rows show up first in the Settings UI.
    pub async fn list_excluded_apps(&self) -> StorageResult<Vec<ExcludedApp>> {
        let rows = sqlx::query(
            "SELECT bundle_id, label, added_at
             FROM excluded_apps
             ORDER BY added_at DESC, bundle_id ASC",
        )
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(ExcludedApp {
                    bundle_id: row.try_get("bundle_id")?,
                    label: row.try_get("label")?,
                    added_at: row.try_get("added_at")?,
                })
            })
            .collect()
    }

    /// Insert a new excluded-app entry. Idempotent — if the `bundle_id`
    /// already exists, the row's `label` is updated to match (so the user
    /// can rename a seeded entry from the Settings UI without first removing
    /// it). Returns `true` if a new row was created, `false` if an existing
    /// row was updated.
    pub async fn add_excluded_app(
        &self,
        bundle_id: &str,
        label: Option<&str>,
    ) -> StorageResult<bool> {
        let trimmed = bundle_id.trim();
        if trimmed.is_empty() {
            return Err(StorageError::InvalidKind(
                "excluded app identifier must not be empty".to_string(),
            ));
        }

        let now = now_ms();
        let result = sqlx::query(
            "INSERT INTO excluded_apps (bundle_id, label, added_at)
             VALUES (?, ?, ?)
             ON CONFLICT(bundle_id) DO UPDATE SET label = excluded.label",
        )
        .bind(trimmed)
        .bind(label)
        .bind(now)
        .execute(self.pool())
        .await?;

        // SQLite reports rows_affected() == 1 for both INSERT and UPDATE on
        // an ON CONFLICT path. Distinguish the two by re-querying added_at
        // — a fresh insert has `now`, an updated row keeps its older value.
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT added_at FROM excluded_apps WHERE bundle_id = ?")
                .bind(trimmed)
                .fetch_optional(self.pool())
                .await?;

        let _ = result;
        Ok(matches!(row, Some((added_at,)) if added_at == now))
    }

    /// Remove an excluded-app entry. Returns `true` if a row was deleted,
    /// `false` if no entry matched. Caller decides how to surface the
    /// no-op (Settings UI just refreshes the list).
    pub async fn remove_excluded_app(&self, bundle_id: &str) -> StorageResult<bool> {
        let result = sqlx::query("DELETE FROM excluded_apps WHERE bundle_id = ?")
            .bind(bundle_id)
            .execute(self.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Re-run a sensitive-content classifier against every live, text-bearing
    /// clip and update the `sensitive` flag in place.
    ///
    /// **Data-preserving by design:** only the `sensitive` column (and
    /// `sync_version`) ever changes. No INSERT, no DELETE, no text-content
    /// rewrites, no blob touches, no FTS5 mutations. Soft-deleted rows are
    /// skipped — they're filtered by the existing partial unique index and
    /// the retention worker eventually GCs them.
    ///
    /// `scan_fn(text) -> bool` is the verdict function. Production callers
    /// inject `crate::clipboard::sensitive::scan(t).is_sensitive()`; tests
    /// pass deterministic stubs.
    ///
    /// Use case: after a regex update (e.g. v0.1.3 added the `sk-proj-`,
    /// `sk-svcacct-`, `sk-admin-` OpenAI formats), historical clips that
    /// were captured under the older regex still carry their old verdict —
    /// this method is the one-tap migration.
    pub async fn resensitize_all<F>(&self, scan_fn: F) -> StorageResult<ResensitizeReport>
    where
        F: Fn(&str) -> bool,
    {
        let rows = sqlx::query(
            "SELECT id, text_content, sensitive
             FROM clips
             WHERE deleted_at IS NULL AND text_content IS NOT NULL",
        )
        .fetch_all(self.pool())
        .await?;

        let mut report = ResensitizeReport::default();

        for row in &rows {
            let id: String = row.try_get("id")?;
            let text: Option<String> = row.try_get("text_content")?;
            let was_sensitive: i64 = row.try_get("sensitive")?;
            let was_sensitive = was_sensitive != 0;

            let Some(text) = text else { continue };

            report.scanned += 1;
            let now_sensitive = scan_fn(&text);

            if now_sensitive == was_sensitive {
                report.unchanged += 1;
                continue;
            }

            sqlx::query(
                "UPDATE clips SET sensitive = ?, sync_version = sync_version + 1
                 WHERE id = ?",
            )
            .bind(now_sensitive as i64)
            .bind(&id)
            .execute(self.pool())
            .await?;

            if now_sensitive {
                report.flagged += 1;
            } else {
                report.unflagged += 1;
            }
        }

        Ok(report)
    }

    /// Re-apply auto-detected labels to every live text clip. The classifier is
    /// re-run per clip; if its key differs from the clip's current auto label,
    /// the old auto label is removed and the new one linked. User-created labels
    /// (`auto_key IS NULL`) are never touched — only the single auto label is.
    ///
    /// Use case: after the classifier's rules change, historical clips pick up
    /// the corrected auto label via one tap in Settings → Privacy.
    pub async fn reclassify_all<F>(&self, classify_fn: F) -> StorageResult<ReclassifyReport>
    where
        F: Fn(&str) -> Option<String>,
    {
        let rows = sqlx::query(
            "SELECT id, text_content FROM clips
             WHERE deleted_at IS NULL AND kind = 'text' AND text_content IS NOT NULL",
        )
        .fetch_all(self.pool())
        .await?;

        let mut report = ReclassifyReport::default();

        for row in &rows {
            let id: String = row.try_get("id")?;
            let text: Option<String> = row.try_get("text_content")?;
            let Some(text) = text else { continue };

            report.scanned += 1;
            let new_key = classify_fn(&text);

            let old_key: Option<String> = sqlx::query_as::<_, (String,)>(
                "SELECT auto_key FROM clip_labels
                 WHERE clip_id = ? AND auto_key IS NOT NULL LIMIT 1",
            )
            .bind(&id)
            .fetch_optional(self.pool())
            .await?
            .map(|(k,)| k);

            if new_key == old_key {
                report.unchanged += 1;
                continue;
            }

            sqlx::query("DELETE FROM clip_labels WHERE clip_id = ? AND auto_key IS NOT NULL")
                .bind(&id)
                .execute(self.pool())
                .await?;

            if let Some(ref k) = new_key {
                self.link_auto_label(&id, k).await?;
            }

            sqlx::query("UPDATE clips SET sync_version = sync_version + 1 WHERE id = ?")
                .bind(&id)
                .execute(self.pool())
                .await?;

            report.changed += 1;
        }

        Ok(report)
    }

    /// Hard-delete EVERY clip row (bypasses tombstone retention).
    ///
    /// Caller is the Settings UI's "Wipe all data" path, which has its own
    /// AlertDialog confirm so the user explicitly opts in. Returns the
    /// number of rows deleted, useful for a "wiped N clips" toast.
    ///
    /// Excluded-apps and settings rows are LEFT INTACT — wiping clipboard
    /// history shouldn't reset the user's hotkey or theme. Caller can
    /// invoke `wipe_settings_and_excluded` separately if a full reset is
    /// what they actually wanted.
    pub async fn wipe_all_clips(&self) -> StorageResult<u64> {
        // The FTS5 trigger fires per-row on DELETE, which is correct but
        // slow on huge tables. For v0.1 with ≤100k clips this is fine; M2
        // follow-up would batch via fts5(delete-all) virtual command.
        let result = sqlx::query("DELETE FROM clips")
            .execute(self.pool())
            .await?;
        Ok(result.rows_affected())
    }
}

/// Outcome of `resensitize_all`. UI surfaces this as a toast:
/// "Scanned N clips: M newly flagged, K unflagged, L unchanged."
///
/// `Default` is derived so the storage method can accumulate counters
/// idiomatically.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResensitizeReport {
    /// Total live, text-bearing clips processed.
    pub scanned: i64,
    /// Rows that flipped `sensitive=0 → 1` (newly detected by an updated regex).
    pub flagged: i64,
    /// Rows that flipped `sensitive=1 → 0` (regex was loosened — rare).
    pub unflagged: i64,
    /// Rows whose verdict matched what was already on disk — no UPDATE issued.
    pub unchanged: i64,
}

/// Outcome of `reclassify_all`. UI surfaces this as a toast:
/// "Scanned N clips: M re-labeled."
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReclassifyReport {
    /// Total live text clips processed.
    pub scanned: i64,
    /// Clips whose auto label changed (added, removed, or swapped).
    pub changed: i64,
    /// Clips whose auto label already matched — no change.
    pub unchanged: i64,
}

/// One label in the vocabulary, with a representative `auto_key` and how many
/// live clips carry it. Surfaced to the popup filter chips + add-label
/// autocomplete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelInfo {
    pub name: String,
    pub auto_key: Option<String>,
    pub count: i64,
}

/// One row from the `excluded_apps` table, surfaced to the Settings UI.
///
/// `added_at` is unix milliseconds; the seed migration uses
/// `strftime('%s','now') * 1000` so all rows have a timestamp from day one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcludedApp {
    pub bundle_id: String,
    pub label: Option<String>,
    pub added_at: i64,
}

pub(crate) fn row_to_clip(row: &sqlx::sqlite::SqliteRow) -> StorageResult<Clip> {
    Ok(Clip {
        id: row.try_get("id")?,
        kind: row.try_get::<&str, _>("kind")?.parse::<ClipKind>()?,
        content_hash: row.try_get("content_hash")?,
        text_content: row.try_get("text_content")?,
        blob_path: row.try_get("blob_path")?,
        size_bytes: row.try_get("size_bytes")?,
        source_app: row.try_get("source_app")?,
        source_url: row.try_get("source_url")?,
        source_window_title: row.try_get("source_window_title")?,
        created_at: row.try_get("created_at")?,
        pinned: row.try_get::<i64, _>("pinned")? != 0,
        sensitive: row.try_get::<i64, _>("sensitive")? != 0,
        title: row.try_get("title")?,
        labels: parse_labels(row.try_get::<Option<String>, _>("labels")?),
    })
}

/// Parse the `json_group_array(json_object('name',…,'autoKey',…))` payload into
/// a `Vec<Label>`. `None` / empty / `"[]"` → empty list; malformed JSON is
/// treated as no labels rather than failing the whole read.
fn parse_labels(raw: Option<String>) -> Vec<Label> {
    match raw {
        Some(s) if !s.is_empty() && s != "[]" => serde_json::from_str(&s).unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_text(body: &str, hash: &str) -> NewClip {
        NewClip {
            kind: ClipKind::Text,
            content_hash: hash.to_string(),
            text_content: Some(body.to_string()),
            blob_path: None,
            size_bytes: body.len() as i64,
            source_app: Some("test.exe".to_string()),
            source_url: None,
            source_window_title: None,
            sensitive: false,
        }
    }

    #[tokio::test]
    async fn insert_then_get() {
        let s = Storage::open_in_memory().await.unwrap();
        let outcome = s
            .insert_clip(sample_text("hello world", "h1"))
            .await
            .unwrap();
        let id = match outcome {
            InsertOutcome::Inserted { id } => id,
            other => panic!("expected Inserted, got {other:?}"),
        };
        let clip = s.get_clip(&id).await.unwrap();
        assert_eq!(clip.text_content.as_deref(), Some("hello world"));
        assert_eq!(clip.kind, ClipKind::Text);
        assert!(!clip.pinned);
        assert!(!clip.sensitive);
    }

    #[tokio::test]
    async fn dedup_bumps_existing_row() {
        let s = Storage::open_in_memory().await.unwrap();
        let first = match s.insert_clip(sample_text("x", "samehash")).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        // Sleep just enough that created_at increments by ≥1ms.
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let outcome = s.insert_clip(sample_text("x", "samehash")).await.unwrap();
        match outcome {
            InsertOutcome::Bumped { id } => assert_eq!(id, first),
            other => panic!("expected Bumped, got {other:?}"),
        }
        assert_eq!(s.count_live().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn list_returns_pinned_first() {
        let s = Storage::open_in_memory().await.unwrap();
        let a = match s.insert_clip(sample_text("a", "ha")).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        let _b = s.insert_clip(sample_text("b", "hb")).await.unwrap();
        let _c = s.insert_clip(sample_text("c", "hc")).await.unwrap();

        s.pin_clip(&a, true).await.unwrap();
        let list = s.list_clips(10, 0).await.unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].id, a, "pinned clip must come first");
    }

    #[tokio::test]
    async fn soft_delete_hides_from_list() {
        let s = Storage::open_in_memory().await.unwrap();
        let id = match s.insert_clip(sample_text("doomed", "hd")).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        s.soft_delete(&id).await.unwrap();
        assert_eq!(s.count_live().await.unwrap(), 0);
        let err = s.get_clip(&id).await.unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[tokio::test]
    async fn dedup_after_soft_delete_creates_new_row() {
        // After soft delete, the partial unique index frees the hash slot
        // — a re-paste of the same content makes a brand-new row.
        let s = Storage::open_in_memory().await.unwrap();
        let first = match s.insert_clip(sample_text("z", "hz")).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        s.soft_delete(&first).await.unwrap();

        let outcome = s.insert_clip(sample_text("z", "hz")).await.unwrap();
        match outcome {
            InsertOutcome::Inserted { id } => assert_ne!(id, first),
            other => panic!("expected Inserted (new row), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn pin_unknown_id_returns_not_found() {
        let s = Storage::open_in_memory().await.unwrap();
        let err = s.pin_clip("does-not-exist", true).await.unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[tokio::test]
    async fn is_app_excluded_loads_seed_and_matches_verbatim() {
        let s = Storage::open_in_memory().await.unwrap();

        // The migration seeds a non-empty list — we don't bind the test to
        // any specific vendor name, just verify the seed loaded.
        let seed_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM excluded_apps")
            .fetch_one(s.pool())
            .await
            .unwrap();
        assert!(seed_count.0 > 0, "migration should seed at least one entry");

        // Insert a synthetic excluded entry and verify the matcher logic.
        sqlx::query("INSERT INTO excluded_apps (bundle_id, label, added_at) VALUES (?, ?, 0)")
            .bind("Test-Excluded.exe")
            .bind("test entry")
            .execute(s.pool())
            .await
            .unwrap();

        assert!(s.is_app_excluded("Test-Excluded.exe").await.unwrap());
        assert!(!s.is_app_excluded("Other.exe").await.unwrap());
        // Case-sensitive: lowercase variant must NOT match.
        assert!(!s.is_app_excluded("test-excluded.exe").await.unwrap());
    }

    #[tokio::test]
    async fn excluded_apps_list_add_remove_round_trip() {
        let s = Storage::open_in_memory().await.unwrap();

        // Migration seed loaded.
        let initial = s.list_excluded_apps().await.unwrap();
        let initial_count = initial.len();
        assert!(initial_count > 0, "migration should seed entries");

        // Add a brand-new entry.
        let inserted = s
            .add_excluded_app("VaultApp.exe", Some("My Test Vault"))
            .await
            .unwrap();
        assert!(inserted, "first add should report inserted=true");

        let after_add = s.list_excluded_apps().await.unwrap();
        assert_eq!(after_add.len(), initial_count + 1);
        // Most recent entry is first (ORDER BY added_at DESC).
        assert_eq!(after_add[0].bundle_id, "VaultApp.exe");
        assert_eq!(after_add[0].label.as_deref(), Some("My Test Vault"));

        // Re-adding same bundle_id with a different label updates the row.
        let inserted_again = s
            .add_excluded_app("VaultApp.exe", Some("Renamed"))
            .await
            .unwrap();
        assert!(!inserted_again, "second add should report inserted=false");
        let after_update = s.list_excluded_apps().await.unwrap();
        assert_eq!(after_update.len(), initial_count + 1, "no duplicate row");
        let updated_row = after_update
            .iter()
            .find(|e| e.bundle_id == "VaultApp.exe")
            .expect("entry must still exist");
        assert_eq!(updated_row.label.as_deref(), Some("Renamed"));

        // Remove returns true the first time, false the second.
        assert!(s.remove_excluded_app("VaultApp.exe").await.unwrap());
        assert!(!s.remove_excluded_app("VaultApp.exe").await.unwrap());

        let after_remove = s.list_excluded_apps().await.unwrap();
        assert_eq!(after_remove.len(), initial_count);
    }

    #[tokio::test]
    async fn wipe_all_clips_clears_clips_but_preserves_settings_and_exclusions() {
        let s = Storage::open_in_memory().await.unwrap();

        // Seed three clips.
        for (i, hash) in ["h1", "h2", "h3"].iter().enumerate() {
            s.insert_clip(sample_text(&format!("body{i}"), hash))
                .await
                .unwrap();
        }
        assert_eq!(s.count_live().await.unwrap(), 3);

        // Settings + exclusions baseline.
        s.set_setting("theme", "dark").await.unwrap();
        let excluded_before = s.list_excluded_apps().await.unwrap();
        let excluded_count_before = excluded_before.len();

        // Wipe.
        let wiped = s.wipe_all_clips().await.unwrap();
        assert_eq!(wiped, 3, "should delete all three clips");
        assert_eq!(s.count_live().await.unwrap(), 0);

        // Settings + exclusions intact.
        assert_eq!(
            s.get_setting("theme").await.unwrap().as_deref(),
            Some("dark")
        );
        let excluded_after = s.list_excluded_apps().await.unwrap();
        assert_eq!(excluded_after.len(), excluded_count_before);

        // Re-inserting the same hashes works (no orphan tombstone blocks).
        let outcome = s.insert_clip(sample_text("again", "h1")).await.unwrap();
        assert!(matches!(outcome, InsertOutcome::Inserted { .. }));
    }

    #[tokio::test]
    async fn excluded_apps_rejects_empty_identifier() {
        let s = Storage::open_in_memory().await.unwrap();
        let err = s.add_excluded_app("   ", None).await.unwrap_err();
        assert!(
            matches!(err, StorageError::InvalidKind(_)),
            "expected InvalidKind, got {err:?}"
        );
    }

    // ---------------- resensitize_all ----------------

    #[tokio::test]
    async fn resensitize_all_flips_only_changed_rows() {
        let s = Storage::open_in_memory().await.unwrap();

        // 1 benign clip — should stay sensitive=false.
        let benign = match s
            .insert_clip(sample_text("hello world", "h_benign"))
            .await
            .unwrap()
        {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };

        // 1 clip that the OLD regex missed (simulates the v0.1.2 sk-proj bug):
        // text contains a "secret-y" marker but DB says sensitive=false.
        let bug_case = match s
            .insert_clip(sample_text("contains secret_xyz token", "h_bug"))
            .await
            .unwrap()
        {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };

        // 1 clip already correctly flagged.
        let mut already = sample_text("AKIA-fake-already-flagged", "h_already");
        already.sensitive = true;
        let already_id = match s.insert_clip(already).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };

        // Stub scanner: flags texts containing "secret_" or "AKIA".
        let report = s
            .resensitize_all(|text| text.contains("secret_") || text.contains("AKIA"))
            .await
            .unwrap();

        assert_eq!(report.scanned, 3);
        assert_eq!(
            report.flagged, 1,
            "only the bug-case row should flip 0→1: {:?}",
            report
        );
        assert_eq!(report.unflagged, 0);
        assert_eq!(
            report.unchanged, 2,
            "benign + already-flagged stay put: {:?}",
            report
        );

        // Data preservation: count unchanged, every row still readable, text intact.
        assert_eq!(s.count_live().await.unwrap(), 3);
        let benign_clip = s.get_clip(&benign).await.unwrap();
        let flipped_clip = s.get_clip(&bug_case).await.unwrap();
        let already_clip = s.get_clip(&already_id).await.unwrap();
        assert!(!benign_clip.sensitive);
        assert!(flipped_clip.sensitive, "newly flagged row");
        assert!(already_clip.sensitive);
        assert_eq!(
            benign_clip.text_content.as_deref(),
            Some("hello world"),
            "text content must not be mutated"
        );
        assert_eq!(
            flipped_clip.text_content.as_deref(),
            Some("contains secret_xyz token"),
            "text content must not be mutated"
        );
    }

    #[tokio::test]
    async fn resensitize_all_is_idempotent() {
        let s = Storage::open_in_memory().await.unwrap();
        s.insert_clip(sample_text("contains secret_zzz", "h1"))
            .await
            .unwrap();

        let scan = |t: &str| t.contains("secret_");
        let first = s.resensitize_all(scan).await.unwrap();
        let second = s.resensitize_all(scan).await.unwrap();

        assert_eq!(first.flagged, 1);
        assert_eq!(first.unchanged, 0);
        // Second run sees the row already at sensitive=1 → no UPDATE issued.
        assert_eq!(second.flagged, 0);
        assert_eq!(second.unflagged, 0);
        assert_eq!(second.unchanged, 1);
    }

    #[tokio::test]
    async fn resensitize_all_skips_soft_deleted_rows() {
        let s = Storage::open_in_memory().await.unwrap();
        let live_id = match s
            .insert_clip(sample_text("contains secret_live", "h_live"))
            .await
            .unwrap()
        {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        let dead_id = match s
            .insert_clip(sample_text("contains secret_dead", "h_dead"))
            .await
            .unwrap()
        {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        s.soft_delete(&dead_id).await.unwrap();

        let report = s.resensitize_all(|t| t.contains("secret_")).await.unwrap();

        assert_eq!(
            report.scanned, 1,
            "soft-deleted rows must not be touched: {:?}",
            report
        );
        assert_eq!(report.flagged, 1);

        // Live row updated; dead row's row state is undisturbed (still tombstoned).
        let live = s.get_clip(&live_id).await.unwrap();
        assert!(live.sensitive);
        assert!(s.get_clip(&dead_id).await.is_err()); // NotFound — still soft-deleted
    }

    #[tokio::test]
    async fn resensitize_all_can_unflag_when_pattern_loosens() {
        // Edge case: a future regex update REMOVES a pattern. Existing rows
        // flagged by the old version should be able to flip back. This is
        // mainly a contract test — we don't expect to use it often, but the
        // API should support it.
        let s = Storage::open_in_memory().await.unwrap();
        let mut paranoid = sample_text("plain text", "h_paranoid");
        paranoid.sensitive = true; // wrongly flagged by an over-eager old regex
        s.insert_clip(paranoid).await.unwrap();

        // New scanner says nothing is sensitive.
        let report = s.resensitize_all(|_| false).await.unwrap();
        assert_eq!(report.unflagged, 1);
        assert_eq!(report.flagged, 0);

        // Row still exists (data preserved).
        assert_eq!(s.count_live().await.unwrap(), 1);
    }

    // ---------------- settings ----------------

    #[tokio::test]
    async fn settings_get_and_upsert() {
        let s = Storage::open_in_memory().await.unwrap();

        // Seeded keys come back from the migration.
        assert_eq!(
            s.get_setting("theme").await.unwrap().as_deref(),
            Some("system")
        );
        assert_eq!(
            s.get_setting("hotkey").await.unwrap().as_deref(),
            Some("Ctrl+Alt+V")
        );
        assert!(s.get_setting("does-not-exist").await.unwrap().is_none());

        // First set inserts.
        s.set_setting("theme", "dark").await.unwrap();
        assert_eq!(
            s.get_setting("theme").await.unwrap().as_deref(),
            Some("dark")
        );

        // Second set overwrites.
        s.set_setting("theme", "light").await.unwrap();
        assert_eq!(
            s.get_setting("theme").await.unwrap().as_deref(),
            Some("light")
        );

        // Brand-new key works too.
        s.set_setting("brand_new_key", "yes").await.unwrap();
        assert_eq!(
            s.get_setting("brand_new_key").await.unwrap().as_deref(),
            Some("yes")
        );
    }

    // ---------------- labels: auto-detection ----------------

    #[tokio::test]
    async fn link_auto_label_seeds_label() {
        let s = Storage::open_in_memory().await.unwrap();
        let id = match s
            .insert_clip(sample_text("https://example.com", "h_url"))
            .await
            .unwrap()
        {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        s.link_auto_label(&id, "url").await.unwrap();
        let labels = s.get_clip(&id).await.unwrap().labels;
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].name, "Bağlantı");
        assert_eq!(labels[0].auto_key.as_deref(), Some("url"));
    }

    #[tokio::test]
    async fn reclassify_all_relinks_auto_labels_keeping_custom() {
        let s = Storage::open_in_memory().await.unwrap();
        let id = match s
            .insert_clip(sample_text("https://a.example", "h1"))
            .await
            .unwrap()
        {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        s.add_label(&id, "iş").await.unwrap(); // custom label

        let scan = |t: &str| {
            if t.starts_with("http") {
                Some("url".to_string())
            } else {
                None
            }
        };
        let report = s.reclassify_all(scan).await.unwrap();
        assert_eq!(report.scanned, 1);
        assert_eq!(report.changed, 1, "auto label added");

        let mut names: Vec<String> = s
            .get_clip(&id)
            .await
            .unwrap()
            .labels
            .into_iter()
            .map(|l| l.name)
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec!["Bağlantı".to_string(), "iş".to_string()],
            "custom label preserved alongside the auto label"
        );

        // Second run is idempotent.
        let again = s.reclassify_all(scan).await.unwrap();
        assert_eq!(again.changed, 0);
        assert_eq!(again.unchanged, 1);
    }

    // ---------------- title (Phase 1) ----------------

    #[tokio::test]
    async fn set_and_clear_title() {
        let s = Storage::open_in_memory().await.unwrap();
        let id = match s.insert_clip(sample_text("body", "ht")).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        assert!(s.get_clip(&id).await.unwrap().title.is_none());

        s.set_clip_title(&id, Some("  My Note  ")).await.unwrap();
        assert_eq!(
            s.get_clip(&id).await.unwrap().title.as_deref(),
            Some("My Note"),
            "title is trimmed"
        );

        s.set_clip_title(&id, Some("   ")).await.unwrap();
        assert!(
            s.get_clip(&id).await.unwrap().title.is_none(),
            "whitespace-only clears the title"
        );
    }

    #[tokio::test]
    async fn set_title_unknown_id_is_not_found() {
        let s = Storage::open_in_memory().await.unwrap();
        let err = s.set_clip_title("nope", Some("x")).await.unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[tokio::test]
    async fn title_is_searchable_via_fts() {
        // A clip whose BODY doesn't contain the word, but whose TITLE does.
        let s = Storage::open_in_memory().await.unwrap();
        let id = match s
            .insert_clip(sample_text("lorem ipsum dolor", "hfts"))
            .await
            .unwrap()
        {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        assert!(s.search_clips("fatura", 50).await.unwrap().is_empty());
        s.set_clip_title(&id, Some("Fatura notu")).await.unwrap();
        let hits = s.search_clips("fatura", 50).await.unwrap();
        assert_eq!(hits.len(), 1, "title word should be found by search");
    }

    // ---------------- labels: manual add / remove / rename ----------------

    #[tokio::test]
    async fn add_and_remove_labels() {
        let s = Storage::open_in_memory().await.unwrap();
        let id = match s.insert_clip(sample_text("body", "hlm")).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        s.add_label(&id, " iş ").await.unwrap(); // trimmed
        s.add_label(&id, "müşteri").await.unwrap();
        s.add_label(&id, "iş").await.unwrap(); // duplicate → no-op

        let mut names: Vec<String> = s
            .get_clip(&id)
            .await
            .unwrap()
            .labels
            .into_iter()
            .map(|l| l.name)
            .collect();
        names.sort();
        assert_eq!(names, vec!["iş".to_string(), "müşteri".to_string()]);

        s.remove_label(&id, "iş").await.unwrap();
        let names: Vec<String> = s
            .get_clip(&id)
            .await
            .unwrap()
            .labels
            .into_iter()
            .map(|l| l.name)
            .collect();
        assert_eq!(names, vec!["müşteri".to_string()]);
    }

    #[tokio::test]
    async fn add_label_rejects_bad_input() {
        let s = Storage::open_in_memory().await.unwrap();
        let id = match s.insert_clip(sample_text("body", "hlc")).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        assert!(matches!(
            s.add_label(&id, "   ").await.unwrap_err(),
            StorageError::InvalidKind(_)
        ));
        assert!(matches!(
            s.add_label(&id, "a\tb").await.unwrap_err(),
            StorageError::InvalidKind(_)
        ));
        assert!(matches!(
            s.add_label("nope", "x").await.unwrap_err(),
            StorageError::NotFound(_)
        ));
    }

    #[tokio::test]
    async fn rename_label_is_global_and_preserves_auto_key() {
        let s = Storage::open_in_memory().await.unwrap();
        let a = match s.insert_clip(sample_text("a", "hra")).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        let b = match s.insert_clip(sample_text("b", "hrb")).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        s.link_auto_label(&a, "url").await.unwrap();
        s.link_auto_label(&b, "url").await.unwrap();

        s.rename_label("Bağlantı", "Web").await.unwrap();

        for id in [&a, &b] {
            let labels = s.get_clip(id).await.unwrap().labels;
            assert_eq!(labels.len(), 1);
            assert_eq!(labels[0].name, "Web", "renamed globally");
            assert_eq!(
                labels[0].auto_key.as_deref(),
                Some("url"),
                "auto_key preserved across rename"
            );
        }

        // Future auto-detection respects the rename (no duplicate "Bağlantı").
        let c = match s.insert_clip(sample_text("c", "hrc")).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        s.link_auto_label(&c, "url").await.unwrap();
        assert_eq!(s.get_clip(&c).await.unwrap().labels[0].name, "Web");
    }

    #[tokio::test]
    async fn list_labels_reports_live_counts() {
        let s = Storage::open_in_memory().await.unwrap();
        let a = match s.insert_clip(sample_text("a", "hca")).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        let b = match s.insert_clip(sample_text("b", "hcb")).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        s.add_label(&a, "ortak").await.unwrap();
        s.add_label(&b, "ortak").await.unwrap();
        s.add_label(&a, "tek").await.unwrap();

        let labels = s.list_labels().await.unwrap();
        assert_eq!(labels[0].name, "ortak", "most-used first");
        assert_eq!(labels[0].count, 2);
        assert_eq!(labels.iter().find(|l| l.name == "tek").unwrap().count, 1);

        s.soft_delete(&b).await.unwrap();
        let labels = s.list_labels().await.unwrap();
        assert_eq!(labels.iter().find(|l| l.name == "ortak").unwrap().count, 1);
    }

    #[tokio::test]
    async fn labels_cascade_on_hard_delete() {
        let s = Storage::open_in_memory().await.unwrap();
        let id = match s.insert_clip(sample_text("body", "hld")).await.unwrap() {
            InsertOutcome::Inserted { id } => id,
            _ => unreachable!(),
        };
        s.add_label(&id, "x").await.unwrap();
        let before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM clip_labels")
            .fetch_one(s.pool())
            .await
            .unwrap();
        assert_eq!(before.0, 1);

        s.wipe_all_clips().await.unwrap();
        let after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM clip_labels")
            .fetch_one(s.pool())
            .await
            .unwrap();
        assert_eq!(after.0, 0, "clip_labels cascade on hard delete");
    }
}
