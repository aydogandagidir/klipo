//! Klipo local storage — SQLite + FTS5.
//!
//! Public surface:
//!   - `Storage::open(path)` opens or creates the DB and runs pending migrations.
//!   - `Storage::insert_clip`, `list_clips`, `search_clips`, `pin`, `soft_delete`
//!     for the clip CRUD surface used by both the watcher (M3+) and IPC commands.
//!   - `Storage::blob_root` / `Storage::resolve_blob` for the blob layout used
//!     by image / file clips (M3.2).
//!   - `Storage::pool()` for diagnostics/tests; production code should not need it.
//!
//! Concurrency: the underlying `SqlitePool` is `Send + Sync`. We hand `Storage`
//! to Tauri via `app.manage()` and clone-cheap (Arc internally) handles to
//! whichever module needs DB access.

pub mod blob;
pub mod clips;
pub mod error;
pub mod search;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

pub use clips::{Clip, ClipKind, NewClip};
pub use error::{StorageError, StorageResult};

// IMPORTANT: never edit a migration that has shipped — even comment-only
// changes break the SHA-384 checksum sqlx records in `_sqlx_migrations`,
// and the next launch will fail with `Migrate(VersionMismatch(N))` for
// every existing user. Add a `00X_*.sql` file instead.
//
// Note on the `001_initial.sql` excluded-apps seed: the literal `.exe`
// names and `com.*` bundle ids in the INSERT are the EXACT strings the OS
// reports for each process (`QueryFullProcessImageName` on Windows,
// `NSRunningApplication.bundleIdentifier` on macOS). They are functional
// pattern strings, not brand mentions; removing them silently disables
// the security feature for the corresponding tool.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./src/storage/migrations");

/// Application storage handle. Cheaply cloneable (`Arc<SqlitePool>` internally).
#[derive(Clone)]
pub struct Storage {
    inner: Arc<StorageInner>,
}

struct StorageInner {
    pool: SqlitePool,
    /// Absolute path to the on-disk DB file. `None` when opened in-memory.
    db_path: Option<PathBuf>,
}

impl Storage {
    /// Open or create a SQLite database at `path`, running pending migrations.
    ///
    /// The DB is opened with WAL + synchronous=NORMAL to fit the perf budget
    /// (`docs/perf-budget.md` §6). Caller is responsible for ensuring the
    /// directory exists.
    pub async fn open(path: impl AsRef<Path>) -> StorageResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let opts = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self {
            inner: Arc::new(StorageInner {
                pool,
                db_path: Some(path),
            }),
        })
    }

    /// In-memory variant for unit tests.
    #[cfg(test)]
    pub async fn open_in_memory() -> StorageResult<Self> {
        let opts = SqliteConnectOptions::new()
            .in_memory(true)
            .journal_mode(SqliteJournalMode::Memory)
            .synchronous(SqliteSynchronous::Off)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1) // single connection — in-memory DB is per-conn
            .connect_with(opts)
            .await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self {
            inner: Arc::new(StorageInner {
                pool,
                db_path: None,
            }),
        })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.inner.pool
    }

    /// Absolute path to the directory containing the SQLite database. Used
    /// by `blob.rs` to resolve `<db_dir>/blobs/...` and `<db_dir>/thumbs/...`.
    /// Returns `None` for in-memory databases.
    pub fn db_dir(&self) -> Option<PathBuf> {
        self.inner
            .db_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn open_in_memory_runs_migrations() {
        // schema_version is bumped by each migration that touches it. Latest
        // is `005_labels.sql` → "5".
        let storage = Storage::open_in_memory().await.expect("open in memory");
        let row: (String,) =
            sqlx::query_as("SELECT value FROM settings WHERE key='schema_version'")
                .fetch_one(storage.pool())
                .await
                .expect("query schema_version");
        assert_eq!(row.0, "5");
    }

    #[tokio::test]
    async fn default_excluded_apps_seeded() {
        let storage = Storage::open_in_memory().await.unwrap();
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM excluded_apps")
            .fetch_one(storage.pool())
            .await
            .unwrap();
        assert!(count.0 >= 8, "expected at least 8 default excluded apps");
    }

    #[tokio::test]
    async fn fts_table_exists() {
        let storage = Storage::open_in_memory().await.unwrap();
        sqlx::query("SELECT count(*) FROM clips_fts")
            .execute(storage.pool())
            .await
            .expect("clips_fts virtual table queryable");
    }

    #[tokio::test]
    async fn db_dir_resolves_for_disk_storage() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("k.db");
        let storage = Storage::open(&path).await.unwrap();
        assert_eq!(storage.db_dir().unwrap(), tmp.path());
        assert_eq!(storage.blob_root().unwrap(), tmp.path().join("blobs"));
    }

    #[tokio::test]
    async fn db_dir_is_none_for_in_memory() {
        let storage = Storage::open_in_memory().await.unwrap();
        assert!(storage.db_dir().is_none());
        assert!(storage.blob_root().is_none());
    }
}
