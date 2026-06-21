-- 003_organize.sql
-- "Organize" feature set: per-clip title, user tags, and content category.
--
-- Three additions, one schema bump (mirror of docs/storage.md §2 once updated):
--   1. clips.title          — optional human-readable name for a clip.
--   2. tags + clip_tags     — many-to-many user-defined labels.
--   3. idx_clips_category   — lets the popup filter by the (already-existing
--                             but previously dormant) clips.category column.
--
-- The `category` column itself shipped in 001 but never had a writer; the
-- pipeline now fills it (see clipboard/classify.rs). No ALTER needed for it —
-- only the filter index below.
--
-- FTS: the v0.1 search index covered clips.text_content only. We extend the
-- INSERT/DELETE/UPDATE triggers so that a clip's *title* is searchable too
-- (folded the same Turkish-ASCII way as 002). The triggers now also fire when
-- a row has a title but no text_content (e.g. an image the user named), which
-- the 002 triggers skipped.
--
-- IMPORTANT: never edit a shipped migration — the SHA-384 checksum sqlx stores
-- in _sqlx_migrations would change and every existing user's launch would fail
-- with Migrate(VersionMismatch). Add 004_* instead.

-- 1. Title column. SQLite ADD COLUMN is O(1) (nullable, no default rewrite).
ALTER TABLE clips ADD COLUMN title TEXT;

-- 2. Tags vocabulary + junction. ON DELETE CASCADE means tag links disappear
--    when a clip is *hard*-deleted (retention GC / wipe-all); soft-delete
--    leaves them in place, which is correct — an undeleted clip keeps its tags.
CREATE TABLE tags (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    created_at  INTEGER NOT NULL
);

CREATE TABLE clip_tags (
    clip_id  TEXT NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
    tag_id   TEXT NOT NULL REFERENCES tags(id)  ON DELETE CASCADE,
    added_at INTEGER NOT NULL,
    PRIMARY KEY (clip_id, tag_id)
);

CREATE INDEX idx_clip_tags_tag  ON clip_tags(tag_id);
CREATE INDEX idx_clip_tags_clip ON clip_tags(clip_id);

-- 3. Category filter index. Partial (live rows only) to match the access
--    pattern of the popup, which never lists soft-deleted rows.
CREATE INDEX idx_clips_category
    ON clips(category)
    WHERE deleted_at IS NULL AND category IS NOT NULL;

-- 4. Rebuild the FTS triggers to index fold(title + ' ' + text_content).
--    Same per-letter Turkish fold as 002, applied to the concatenation so
--    `isik` finds a clip titled "Işık notu" as well as one whose body says it.
DROP TRIGGER IF EXISTS clips_ai;
DROP TRIGGER IF EXISTS clips_ad;
DROP TRIGGER IF EXISTS clips_au;

CREATE TRIGGER clips_ai AFTER INSERT ON clips
    WHEN new.text_content IS NOT NULL OR new.title IS NOT NULL BEGIN
        INSERT INTO clips_fts(rowid, text_content) VALUES (
            new.rowid,
            lower(
                replace(replace(replace(replace(replace(replace(
                replace(replace(replace(replace(replace(replace(
                    (coalesce(new.title, '') || ' ' || coalesce(new.text_content, '')),
                    'Ç', 'c'), 'ç', 'c'),
                    'Ğ', 'g'), 'ğ', 'g'),
                    'İ', 'i'), 'ı', 'i'),
                    'Ö', 'o'), 'ö', 'o'),
                    'Ş', 's'), 'ş', 's'),
                    'Ü', 'u'), 'ü', 'u')
            )
        );
    END;

CREATE TRIGGER clips_ad AFTER DELETE ON clips
    WHEN old.text_content IS NOT NULL OR old.title IS NOT NULL BEGIN
        INSERT INTO clips_fts(clips_fts, rowid, text_content) VALUES('delete', old.rowid,
            lower(
                replace(replace(replace(replace(replace(replace(
                replace(replace(replace(replace(replace(replace(
                    (coalesce(old.title, '') || ' ' || coalesce(old.text_content, '')),
                    'Ç', 'c'), 'ç', 'c'),
                    'Ğ', 'g'), 'ğ', 'g'),
                    'İ', 'i'), 'ı', 'i'),
                    'Ö', 'o'), 'ö', 'o'),
                    'Ş', 's'), 'ş', 's'),
                    'Ü', 'u'), 'ü', 'u')
            )
        );
    END;

CREATE TRIGGER clips_au AFTER UPDATE ON clips
    WHEN old.text_content IS NOT NULL OR new.text_content IS NOT NULL
      OR old.title IS NOT NULL OR new.title IS NOT NULL BEGIN
        INSERT INTO clips_fts(clips_fts, rowid, text_content) VALUES('delete', old.rowid,
            lower(
                replace(replace(replace(replace(replace(replace(
                replace(replace(replace(replace(replace(replace(
                    (coalesce(old.title, '') || ' ' || coalesce(old.text_content, '')),
                    'Ç', 'c'), 'ç', 'c'),
                    'Ğ', 'g'), 'ğ', 'g'),
                    'İ', 'i'), 'ı', 'i'),
                    'Ö', 'o'), 'ö', 'o'),
                    'Ş', 's'), 'ş', 's'),
                    'Ü', 'u'), 'ü', 'u')
            )
        );
        INSERT INTO clips_fts(rowid, text_content) VALUES (new.rowid,
            lower(
                replace(replace(replace(replace(replace(replace(
                replace(replace(replace(replace(replace(replace(
                    (coalesce(new.title, '') || ' ' || coalesce(new.text_content, '')),
                    'Ç', 'c'), 'ç', 'c'),
                    'Ğ', 'g'), 'ğ', 'g'),
                    'İ', 'i'), 'ı', 'i'),
                    'Ö', 'o'), 'ö', 'o'),
                    'Ş', 's'), 'ş', 's'),
                    'Ü', 'u'), 'ü', 'u')
            )
        );
    END;

-- Rebuild the index so existing rows are re-emitted through the new expression.
-- (Existing rows have title IS NULL, so their tokens are unchanged — but the
-- rebuild keeps the index definitionally in sync with the trigger output.)
INSERT INTO clips_fts(clips_fts) VALUES('delete-all');

INSERT INTO clips_fts(rowid, text_content)
    SELECT rowid,
        lower(
            replace(replace(replace(replace(replace(replace(
            replace(replace(replace(replace(replace(replace(
                (coalesce(title, '') || ' ' || coalesce(text_content, '')),
                'Ç', 'c'), 'ç', 'c'),
                'Ğ', 'g'), 'ğ', 'g'),
                'İ', 'i'), 'ı', 'i'),
                'Ö', 'o'), 'ö', 'o'),
                'Ş', 's'), 'ş', 's'),
                'Ü', 'u'), 'ü', 'u')
        )
    FROM clips
    WHERE (text_content IS NOT NULL OR title IS NOT NULL) AND deleted_at IS NULL;

UPDATE settings SET value = '3' WHERE key = 'schema_version';
