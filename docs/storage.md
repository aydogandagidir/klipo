# Klipo Storage & Data Lifecycle

**Status:** Draft. Locks before v0.1 ships.
**Audience:** Engineers writing the storage layer; product reviewers deciding default policies.

This doc owns: schema, retention, blob layout, dedup, migrations, backup/export. Crypto-at-rest details live in [`crypto.md`](./crypto.md).

---

## 1. Storage Locations

Per-OS layout. Only Windows is implemented in v0.1; macOS shown for v0.2 design.

### 1.1 Windows

```
%APPDATA%\Klipo\
├─ klipo.db                    # SQLite database (WAL mode)
├─ klipo.db-wal                # WAL log (auto-managed)
├─ klipo.db-shm                # Shared memory file
├─ blobs/                      # Binary content (images, large files)
│   ├─ 0a/
│   │   └─ 0a3f...c2.png       # SHA-256-named, first 2 hex chars sharded
│   ├─ 1f/
│   │   └─ 1f8e...d4.bin
│   └─ ...
├─ thumbs/                     # Cached thumbnails for image clips
│   └─ <sha256>-192.webp
├─ logs/
│   └─ klipo.log               # Rolling, no clipboard content
├─ backups/                    # User-initiated exports
│   └─ klipo-2026-05-04.kpb    # Encrypted backup bundle
└─ keychain/                   # Sealed device keys (sodium-sealed)
    ├─ device.key.enc
    └─ vault.key.enc
```

`%APPDATA%` = `C:\Users\<user>\AppData\Roaming` by default. We respect Windows roaming profile semantics (small files only — blobs go local-only via `%LOCALAPPDATA%` if user enables roaming-aware mode in v1.0).

### 1.2 macOS (v0.2 design)

```
~/Library/Application Support/Klipo/
├─ klipo.db
├─ blobs/
├─ thumbs/
└─ logs/

~/Library/Caches/Klipo/        # OCR cache, ephemeral
└─ ocr/

# Keychain entries:
#   service "klipo.app", account "device-key"
#   service "klipo.app", account "vault-key"
```

### 1.3 Linux (v1.x design — XDG)

```
$XDG_DATA_HOME/klipo/   (defaults to ~/.local/share/klipo)
$XDG_STATE_HOME/klipo/  (logs, ephemeral)
$XDG_CACHE_HOME/klipo/  (thumbs)
```

---

## 2. SQLite Schema

Initial migration (`src-tauri/src/storage/migrations/001_initial.sql`). Reproducing PRD spec verbatim with one addition: HLC reserved column.

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;

CREATE TABLE clips (
    id              TEXT PRIMARY KEY,                      -- UUIDv7
    kind            TEXT NOT NULL CHECK(kind IN ('text','image','file','rtf','html')),
    content_hash    TEXT NOT NULL,                         -- SHA-256 hex
    text_content    TEXT,                                   -- nullable for binary kinds
    blob_path       TEXT,                                   -- relative to APPDATA/Klipo/blobs
    size_bytes      INTEGER NOT NULL,
    source_app      TEXT,                                   -- exe name or bundle id
    source_url      TEXT,                                   -- if browser-sourced
    source_window_title TEXT,                               -- v0.2; helpful for context
    created_at      INTEGER NOT NULL,                       -- unix ms (local clock)
    pinned          INTEGER NOT NULL DEFAULT 0,
    deleted_at      INTEGER,                                -- soft delete (sync tombstone)
    sensitive       INTEGER NOT NULL DEFAULT 0,
    category        TEXT,                                   -- v0.2: AI-assigned or user-tagged
    sync_version    INTEGER NOT NULL DEFAULT 0,             -- bumped on local mutation
    hlc             TEXT                                    -- 16-hex (8-byte) HLC, NULL until v0.3 sync
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

-- FTS5 virtual table for substring + fuzzy search
CREATE VIRTUAL TABLE clips_fts USING fts5(
    text_content,
    content='clips',
    content_rowid='rowid',
    tokenize='unicode61 remove_diacritics 2'
);

-- Triggers to keep FTS in sync
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

-- Default exclusions: a seed list of common password-manager process names
-- (Windows `.exe`) and bundle ids (macOS `com.*`). See
-- `src-tauri/src/storage/migrations/001_initial.sql` for the full inserted set.

-- Default settings
INSERT INTO settings (key, value) VALUES
    ('schema_version', '1'),
    ('history_limit', '10000'),
    ('retention_days_pinned', 'unlimited'),
    ('retention_days_unpinned', '90'),
    ('retention_days_sensitive', '7'),
    ('retention_days_deleted', '30'),
    ('clipboard_poll_interval_ms', '500'),
    ('hotkey', 'Ctrl+Alt+V'),
    ('theme', 'system'),
    ('telemetry', 'off'),
    ('sync', 'off'),
    ('max_blob_mb', '50'),
    ('thumbnail_size_px', '192');
```

---

## 2.1 Organize: title, labels, favorite (003–005)

The "Organize" feature settled over three migrations. Net result:

- **`title`** (003) — an optional per-clip name, folded into the FTS index.
- **labels** (005) — a clip carries MULTIPLE labels (`clip_labels`). Auto-
  detection seeds the first; the user can add, rename (globally), and remove
  them. This replaced the single `category` idea (003 also created free-text
  `tags`/`clip_tags`, dropped by 004; `category` is left dead in the row,
  superseded by `clip_labels`).
- **favorite** — the per-row star toggles the existing `pinned` flag.

```sql
-- 003: per-clip title (+ title-aware FTS triggers).
ALTER TABLE clips ADD COLUMN title TEXT;

-- 005: multi-label system. `auto_key` ties an auto label to a classifier key
-- so detection survives a rename; NULL for user-created labels.
CREATE TABLE clip_labels (
    clip_id  TEXT NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
    name     TEXT NOT NULL,
    auto_key TEXT,                 -- 'url','email',… for auto labels; NULL custom
    added_at INTEGER NOT NULL,
    PRIMARY KEY (clip_id, name)
);
CREATE INDEX idx_clip_labels_name ON clip_labels(name);
CREATE INDEX idx_clip_labels_clip ON clip_labels(clip_id);
-- 005 also migrates clips.category → seed labels, then drops idx_clips_category.
```

Behavioural notes:

- **Auto label on capture.** For `kind='text'` clips the pipeline runs
  `clipboard::classify::classify` (regex/heuristic, fully local — no model, no
  network) and, after insert, links the matching auto label via
  `Storage::link_auto_label`. Default display names come from
  `classify::auto_label_name` (Bağlantı, E-posta, Kod, …); the stable key lives
  in `auto_key`. Keys: `url`, `email`, `phone`, `iban`, `color`, `code`, `json`,
  `number`, `path`.
- **User-managed labels.** `add_label` (custom; inherits `auto_key` if the name
  matches a known auto label so it keeps its color), `remove_label`, and
  `rename_label` (global `UPDATE OR REPLACE`, preserving `auto_key` so future
  auto-detections reuse the renamed label). Each bumps the clip's `sync_version`.
  `Storage::reclassify_all` re-applies auto labels over history, preserving
  user-created ones.
- **Read path.** `get_clip` / `list_clips` / `search_clips` attach a
  `json_group_array(json_object('name', …, 'autoKey', …))` subquery aliased
  `labels`; `row_to_clip` parses it into `Vec<Label>`.
- **Title in FTS.** The `clips_ai/ad/au` triggers (003) index
  `fold(title + ' ' + text_content)` and fire when a row has a title but no body
  (e.g. a named image). So a clip's title is searchable with the same
  Turkish-folded query path as its body. Display uses the original glyphs.
- **Favorite = `pinned`.** The per-row star toggles the existing `pinned` flag:
  favorited clips sort first (`ORDER BY pinned DESC`) and are filterable via the
  popup's "Favoriler" chip. No separate column.

---

## 3. Insert Path

```
1. Receive clipboard event with raw bytes + metadata.
2. Normalize:
     - text/html → both stored, kind=text (HTML in `text_content`, plain in derived view)
     - RTF  → strip to text, store text + raw RTF in `text_content` JSON wrapper
     - image → re-encode to PNG (lossless), strip metadata
     - file → store list of paths as JSON in text_content; kind='file'
3. SHA-256 hash of normalized bytes.
4. Sensitive regex scan (text-kind only).
5. Determine source app via foreground window query.
6. If source_app in excluded_apps → DROP, no DB write, no log.
7. Lookup hash in idx_clips_hash:
     - If exists and deleted_at IS NULL → bump created_at; do not insert duplicate.
     - Else → continue.
8. If kind ∈ {image, file}: write blob to %APPDATA%\Klipo\blobs\<hash[:2]>\<hash>.<ext>.
9. INSERT INTO clips (...) VALUES (...).
10. Triggers update FTS5.
11. Emit Tauri event "clip:new" with new id.
```

Steps 4-9 run on Tokio worker, NOT on the WindowProc thread (Windows) or pasteboard timer thread (macOS).

---

## 4. Dedup Strategy

- **Hash-based unique index** on `content_hash WHERE deleted_at IS NULL`.
- "Same hash" means literal byte equality. Lossy normalization (e.g., RTF → text) is one-way; we hash post-normalization.
- Pasting the same text twice in a row → no new row; existing row gets bumped (`created_at = now`, `sync_version += 1`).
- Image dedup is exact-bit. Two screenshots taken seconds apart at the same coords are usually identical → dedup'd.
- Files with same path + same content (ino+mtime) generate identical blob → dedup. Files with same path but different content → different hash → two rows.

---

## 5. Retention Policy

User-tunable in Settings. Defaults below.

| Cohort | Default Retention | Why |
|---|---|---|
| Pinned | Unlimited | User explicitly said "keep this." |
| Unpinned | 90 days | Balances "I copy URL last month" vs disk usage. |
| Sensitive | 7 days | Strong default to limit exposure. |
| Soft-deleted (tombstoned) | 30 days | Sync tombstone propagation; then physical delete. |

### 5.1 Retention Worker

Runs daily at 03:00 local time (or on app start if more than 24h since last run):

```sql
-- Example: prune unpinned non-sensitive past retention
UPDATE clips
SET deleted_at = strftime('%s','now')*1000,
    sync_version = sync_version + 1
WHERE deleted_at IS NULL
  AND pinned = 0
  AND sensitive = 0
  AND created_at < (strftime('%s','now')*1000 - 90*86400000);

-- Hard-delete tombstones older than 30d
DELETE FROM clips
WHERE deleted_at IS NOT NULL
  AND deleted_at < (strftime('%s','now')*1000 - 30*86400000);
```

After hard delete: orphaned blobs swept by a separate job (compare blobs/ filesystem vs `blob_path` in DB; delete unreferenced; reverse check to detect missing blobs and surface as integrity warning).

### 5.2 History-Limit Pruning

Independent of date-based retention. If `count(clips) > history_limit`, drop oldest unpinned non-sensitive until under limit. Default `history_limit = 10000`.

```sql
DELETE FROM clips
WHERE id IN (
    SELECT id FROM clips
    WHERE deleted_at IS NULL
      AND pinned = 0
      AND sensitive = 0
    ORDER BY created_at ASC
    LIMIT (
        SELECT COUNT(*) FROM clips WHERE deleted_at IS NULL
    ) - (SELECT CAST(value AS INTEGER) FROM settings WHERE key='history_limit')
);
```

(In practice, batched prune so we don't lock for long.)

---

## 6. Blob Layout

```
blobs/
├─ 0a/
│   └─ 0a3f...c2.png            # SHA-256 named, real extension
├─ 1f/
│   └─ 1f8e...d4.bin            # unknown binary → .bin
└─ ...

thumbs/
└─ 0a3f...c2-192.webp          # WebP for size; 192 = px on long edge
```

Two-character hex sharding keeps any one folder under ~16k files even at 4M total (way past our 10k clip target).

**Path stored in DB:** Relative path from `%APPDATA%\Klipo\blobs`, e.g. `0a/0a3f...c2.png`. Absolute paths derived at runtime via Tauri's `path::AppLocalData`.

**Max blob size:** 50MB hard cap. Larger clips drop with toast notification ("Image >50MB skipped"). Cap is a setting.

---

## 7. Migration Strategy

`sqlx::migrate!()` runs on app start. Migrations are forward-only.

```
src-tauri/src/storage/migrations/
├─ 001_initial.sql
├─ 002_turkish_fts.sql           (FTS5 Turkish-ASCII fold; schema_version → 2)
├─ 003_organize.sql              (title + tags + category index + title FTS; schema_version → 3)
├─ 004_drop_tags.sql             (drop tags/clip_tags; schema_version → 4)
├─ 005_labels.sql                (clip_labels multi-label + migrate category; schema_version → 5)
├─ 006_sync_columns.sql          (v0.3 — populate hlc for existing rows)
└─ ...
```

### 7.1 Versioning

`settings.schema_version` tracks the latest applied migration. On startup:

```rust
let current: u32 = get_setting("schema_version").await?.parse()?;
sqlx::migrate!("./src/storage/migrations").run(&pool).await?;
let new: u32 = latest_migration_version();
if new > current {
    set_setting("schema_version", new.to_string()).await?;
    log::info!("Migrated schema {} → {}", current, new);
}
```

### 7.2 Pre-Migration Backup

Before running ANY migration that's not 001:

1. Copy `klipo.db` to `klipo.db.pre-mig-<old_version>.bak`.
2. Run migration.
3. On success, keep backup for 30 days then delete.
4. On failure, abort startup, surface error, instruct user to restore.

This bound is small (DB is ≤200MB even at history_limit). Worth the disk for safety.

### 7.3 v0.3 HLC Backfill

When sync first enables on a vault that has v0.1 data (no HLC):

```
Migration 004:
  ALTER TABLE clips ADD COLUMN hlc TEXT;
  UPDATE clips SET hlc = printf('%016x', (created_at << 16))
    WHERE hlc IS NULL;
  -- Devices catch up via sync; first push wins HLC ordering.
```

---

## 8. Backup & Export

### 8.1 Encrypted Backup Bundle (`.kpb`)

```
backup-2026-05-04.kpb            (zip archive)
├─ manifest.json                  (version, vault_id, created_at, item_count)
├─ klipo.db                       (full SQLite snapshot)
├─ blobs/                         (full blob mirror)
└─ signature.bin                  (Ed25519 signature over manifest)
```

The whole archive is encrypted under a **separate backup key** derived from a backup passphrase the user sets at export time. (Not the vault password — different threat model. User may want to share backup with a sysadmin without giving up vault.)

```
backup_key = Argon2id(backup_passphrase, salt=fresh_random)
ciphertext = XChaCha20Poly1305_encrypt(zip_bytes, backup_key, nonce=fresh_random)

stored = {
    salt, nonce, ciphertext_size, ciphertext
}
```

Restore: prompt for backup passphrase; decrypt; replace current DB after confirmation.

### 8.2 Plaintext JSON Export

For users who want to script processing of their data:

```json
{
  "version": 1,
  "exported_at": "2026-05-04T12:34:56Z",
  "items": [
    {
      "id": "01J9...",
      "kind": "text",
      "text": "...",
      "created_at": 1746355200000,
      "source_app": "Code.exe",
      "pinned": true,
      "category": null
    },
    ...
  ]
}
```

Default toggles in export dialog:
- [ ] Include sensitive items (default OFF)
- [ ] Include images and files (default OFF — they balloon size)
- [x] Include pinned items
- [x] Include settings

Plaintext exports come with a clear warning banner: "This file is unencrypted. Anyone who reads it can see all your clipboard history. Store it securely."

---

## 9. Disk Pressure Handling

When OS reports low disk (<5% free or <500MB):

1. Show non-blocking banner.
2. Pause new image captures (text still allowed).
3. Offer one-click "Free 200MB" that runs:
    - Hard-delete tombstones immediately (instead of 30d wait).
    - Re-encode large images at lower quality (or hard-prune images > X MB).

We never silently delete. Always confirm.

---

## 10. Integrity Checks

On app start (debounced; once per day per machine):

1. `PRAGMA integrity_check` on the SQLite DB. Failure → block startup, surface "DB corrupt, restore from backup?"
2. Walk `blobs/` directory: every file must correspond to a `clips.blob_path`. Orphans go to `blobs/.orphans/` (held 7d, then deleted) — gives recovery window.
3. For 1% random sample: verify file's actual SHA-256 matches its filename. Mismatch → quarantine + surface warning.

---

## 11. Per-OS File-System Considerations

- **Windows:** NTFS handles the sharded layout fine. Defender may scan large blobs on write — we accept the latency hit (<150ms for 2MB).
- **macOS:** APFS clones blobs cheaply if user copies same image twice (we still hash + dedup at our layer; APFS is below us).
- **Network drives:** Detected at startup; if `%APPDATA%` resolves to a network share, warn user (perf, durability). Allow override.

---

## 12. Developer Tooling

- `klipo-cli dump` (v0.2) prints DB stats: total clips, by kind, blob size, retention buckets.
- `klipo-cli vacuum` runs `VACUUM` + orphan blob cleanup on demand.
- `klipo-cli repair` runs integrity checks + offers fixes.

These are debug/admin tools, not exposed in default UI.

---

## 13. Open Decisions

- **SQLCipher opt-in?** v0.2 candidate. Pros: at-rest encryption even if `%APPDATA%` is read by another local user (shouldn't happen on standard Windows perms but defensive). Cons: ~10% perf hit. Decision: **opt-in toggle in v0.2**, default off (relies on OS disk encryption).
- **Sync engine separate worker process or in-app thread?** Worker process gives fault isolation (sync crash doesn't take down popup). In-app simpler. **Decision deferred to v0.3.**
- **Compression of text content?** ZSTD at 1KB threshold could halve text storage. Adds complexity. **Decision: skip for v0.1; revisit at 100k clip user feedback.**
- **iCloud Drive / OneDrive auto-sync the DB?** Strongly NO — corruption guaranteed. We refuse to start if storage path is on iCloud/OneDrive Documents folder; require move.
