//! Klipo Phase A prototype benchmark helpers.
//!
//! Goal: validate that SQLite + FTS5 + sqlx on Windows can meet
//! the perf budget defined in `docs/perf-budget.md`.
//!
//! What lives here:
//!   - DB bootstrap matching the real v0.1 schema (lifted from
//!     `src-tauri/src/storage/migrations/001_initial.sql` once that
//!     file lands; kept in sync manually for now).
//!   - Synthetic clip generators (text, mixed lengths, Turkish corpus).
//!
//! What does NOT live here:
//!   - The actual production storage layer (that's Phase B / `src-tauri/`).
//!   - The Tauri runtime, React frontend, or any clipboard logic.
//!   - Crypto, sync — irrelevant for storage perf measurement.

use rand::distributions::{Alphanumeric, Distribution};
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

/// SQL applied to a fresh DB. Mirror of v0.1 migration 001.
/// Keep in sync with `src-tauri/src/storage/migrations/001_initial.sql` when that lands.
pub const SCHEMA_SQL: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;
PRAGMA foreign_keys = ON;

CREATE TABLE clips (
    id              TEXT PRIMARY KEY,
    kind            TEXT NOT NULL CHECK(kind IN ('text','image','file','rtf','html')),
    content_hash    TEXT NOT NULL,
    text_content    TEXT,
    blob_path       TEXT,
    size_bytes      INTEGER NOT NULL,
    source_app      TEXT,
    source_url      TEXT,
    source_window_title TEXT,
    created_at      INTEGER NOT NULL,
    pinned          INTEGER NOT NULL DEFAULT 0,
    deleted_at      INTEGER,
    sensitive       INTEGER NOT NULL DEFAULT 0,
    category        TEXT,
    sync_version    INTEGER NOT NULL DEFAULT 0,
    hlc             TEXT
);

CREATE INDEX idx_clips_created
    ON clips(created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_clips_pinned
    ON clips(pinned DESC, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX idx_clips_hash
    ON clips(content_hash)
    WHERE deleted_at IS NULL;

CREATE VIRTUAL TABLE clips_fts USING fts5(
    text_content,
    content='clips',
    content_rowid='rowid',
    tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER clips_ai AFTER INSERT ON clips
    WHEN new.text_content IS NOT NULL BEGIN
        INSERT INTO clips_fts(rowid, text_content) VALUES (new.rowid, new.text_content);
    END;

CREATE TRIGGER clips_ad AFTER DELETE ON clips
    WHEN old.text_content IS NOT NULL BEGIN
        INSERT INTO clips_fts(clips_fts, rowid, text_content)
            VALUES('delete', old.rowid, old.text_content);
    END;

CREATE TRIGGER clips_au AFTER UPDATE ON clips
    WHEN old.text_content IS NOT NULL OR new.text_content IS NOT NULL BEGIN
        INSERT INTO clips_fts(clips_fts, rowid, text_content)
            VALUES('delete', old.rowid, old.text_content);
        INSERT INTO clips_fts(rowid, text_content)
            VALUES (new.rowid, new.text_content);
    END;
"#;

pub async fn open_pool(db_path: &str) -> SqlitePool {
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path))
        .expect("valid sqlite url")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .expect("connect");

    for stmt in SCHEMA_SQL.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        sqlx::query(stmt).execute(&pool).await.expect("schema");
    }
    pool
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    format!("{:x}", h.finalize())
}

pub fn uuid_v7() -> String {
    uuid::Uuid::now_v7().to_string()
}

pub struct GeneratedClip {
    pub id: String,
    pub kind: &'static str,
    pub content_hash: String,
    pub text_content: String,
    pub size_bytes: i64,
    pub source_app: &'static str,
    pub created_at: i64,
}

const SOURCE_APPS: &[&str] = &[
    "Code.exe", "chrome.exe", "explorer.exe", "Cursor.exe", "slack.exe",
    "notion.exe", "Discord.exe", "WindowsTerminal.exe", "msedge.exe", "obsidian.exe",
];

/// Generate a random English text clip with realistic length distribution.
pub fn gen_text_clip(rng: &mut StdRng) -> GeneratedClip {
    let len = match rng.gen_range(0..100) {
        0..=49 => rng.gen_range(8..120),    // half are short (URLs, words)
        50..=89 => rng.gen_range(120..1000),// most are paragraph-sized
        _ => rng.gen_range(1000..8000),     // tail: longer pastes
    };
    let body: String = Alphanumeric
        .sample_iter(&mut *rng)
        .take(len)
        .map(char::from)
        .collect();
    GeneratedClip {
        id: uuid_v7(),
        kind: "text",
        content_hash: sha256_hex(body.as_bytes()),
        text_content: body.clone(),
        size_bytes: len as i64,
        source_app: SOURCE_APPS.choose(rng).unwrap(),
        created_at: now_ms() - rng.gen_range(0..86_400_000_i64 * 30), // last 30 days
    }
}

/// Mix Turkish + English real-ish words for FTS5 tokenizer testing.
pub fn gen_turkish_text_clip(rng: &mut StdRng) -> GeneratedClip {
    const TURKISH_WORDS: &[&str] = &[
        "ışık", "Işık", "kalem", "şeker", "ığdır", "İstanbul", "öğretmen",
        "çiçek", "günaydın", "yağmur", "balık", "ülke", "üniversite", "ekmek",
        "kitap", "deniz", "araba", "yıldız", "ağaç", "gözlük",
    ];
    const ENGLISH_WORDS: &[&str] = &[
        "function", "request", "database", "search", "query", "perfect",
        "sample", "system", "result", "context",
    ];
    let n_words = rng.gen_range(8..40);
    let mut words = Vec::with_capacity(n_words);
    for _ in 0..n_words {
        if rng.gen_bool(0.6) {
            words.push(*TURKISH_WORDS.choose(rng).unwrap());
        } else {
            words.push(*ENGLISH_WORDS.choose(rng).unwrap());
        }
    }
    let body = words.join(" ");
    let bytes = body.as_bytes();
    GeneratedClip {
        id: uuid_v7(),
        kind: "text",
        content_hash: sha256_hex(bytes),
        text_content: body.clone(),
        size_bytes: bytes.len() as i64,
        source_app: SOURCE_APPS.choose(rng).unwrap(),
        created_at: now_ms() - rng.gen_range(0..86_400_000_i64 * 30),
    }
}

pub async fn insert_clip(pool: &SqlitePool, clip: &GeneratedClip) {
    sqlx::query(
        "INSERT INTO clips (id, kind, content_hash, text_content, size_bytes, source_app, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&clip.id)
    .bind(clip.kind)
    .bind(&clip.content_hash)
    .bind(&clip.text_content)
    .bind(clip.size_bytes)
    .bind(clip.source_app)
    .bind(clip.created_at)
    .execute(pool)
    .await
    .expect("insert");
}

/// Bulk insert helper using a single transaction.
pub async fn bulk_insert(pool: &SqlitePool, clips: &[GeneratedClip]) {
    let mut tx = pool.begin().await.expect("begin tx");
    for clip in clips {
        sqlx::query(
            "INSERT INTO clips (id, kind, content_hash, text_content, size_bytes, source_app, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&clip.id)
        .bind(clip.kind)
        .bind(&clip.content_hash)
        .bind(&clip.text_content)
        .bind(clip.size_bytes)
        .bind(clip.source_app)
        .bind(clip.created_at)
        .execute(&mut *tx)
        .await
        .expect("insert");
    }
    tx.commit().await.expect("commit");
}

pub fn seeded_rng(seed: u64) -> StdRng {
    StdRng::seed_from_u64(seed)
}

/// Convenience: a fresh ephemeral DB per benchmark run.
pub async fn fresh_db(name: &str) -> (tempfile::TempDir, SqlitePool) {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join(format!("{}.db", name));
    let pool = open_pool(path.to_str().unwrap()).await;
    (dir, pool)
}
