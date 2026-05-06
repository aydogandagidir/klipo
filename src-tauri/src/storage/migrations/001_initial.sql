-- 001_initial.sql
-- Klipo v0.1 storage schema. Mirror of docs/storage.md §2.
-- DO NOT alter once shipped — write a new migration file (002_*) instead.

PRAGMA foreign_keys = ON;
PRAGMA temp_store = MEMORY;

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

CREATE TABLE excluded_apps (
    bundle_id TEXT PRIMARY KEY,
    label     TEXT,
    added_at  INTEGER NOT NULL
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT INTO excluded_apps (bundle_id, label, added_at) VALUES
    ('1Password.exe',                'Password Manager',   strftime('%s','now')*1000),
    ('Bitwarden.exe',                'Password Manager',   strftime('%s','now')*1000),
    ('KeePassXC.exe',                'Password Manager',   strftime('%s','now')*1000),
    ('keepass.exe',                  'Password Manager',   strftime('%s','now')*1000),
    ('Dashlane.exe',                 'Password Manager',   strftime('%s','now')*1000),
    ('LastPass.exe',                 'Password Manager',   strftime('%s','now')*1000),
    ('com.1password.1password',      'Password Manager',   strftime('%s','now')*1000),
    ('com.1password.1password7',     'Password Manager',   strftime('%s','now')*1000),
    ('com.bitwarden.desktop',        'Password Manager',   strftime('%s','now')*1000),
    ('org.keepassxc.keepassxc',      'Password Manager',   strftime('%s','now')*1000);

INSERT INTO settings (key, value) VALUES
    ('schema_version',           '1'),
    ('history_limit',            '10000'),
    ('retention_days_unpinned',  '90'),
    ('retention_days_sensitive', '7'),
    ('retention_days_deleted',   '30'),
    ('clipboard_poll_interval_ms', '500'),
    ('hotkey',                   'Ctrl+Alt+V'),
    ('theme',                    'system'),
    ('telemetry',                'off'),
    ('sync',                     'off'),
    ('max_blob_mb',              '50'),
    ('thumbnail_size_px',        '192');
