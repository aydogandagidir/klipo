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
    pub category: Option<String>,
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
                    created_at, pinned, sensitive, category
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
                    created_at, pinned, sensitive, category
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

fn row_to_clip(row: &sqlx::sqlite::SqliteRow) -> StorageResult<Clip> {
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
        category: row.try_get("category")?,
    })
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
}
