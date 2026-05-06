-- 002_turkish_fts.sql
-- Make FTS5 case-fold Turkish letters (ı/İ/ş/Ş/ğ/Ğ/ü/Ü/ö/Ö/ç/Ç) so that
-- typing `isik` finds `ışık`, `ogretmen` finds `öğretmen`, etc.
--
-- Why: SQLite `unicode61 remove_diacritics 2` decomposes letters with combining
-- marks (`ü = u + ¨` → `u`) but `ı` (U+0131) is its OWN code point — there's no
-- decomposable form. Same problem for ş/ğ/ç (cedilla / breve in some forms,
-- standalone glyphs in others depending on input source).
--
-- Strategy: store the Turkish-ASCII-folded text in the FTS5 index instead of
-- the original. Query side (search.rs) folds the user's query the same way,
-- so both ends speak the same alphabet. Display still uses `clips.text_content`
-- with original glyphs intact.
--
-- Idempotent: dropping triggers is `IF EXISTS`; rebuilding FTS index works
-- against any prior state.

DROP TRIGGER IF EXISTS clips_ai;
DROP TRIGGER IF EXISTS clips_ad;
DROP TRIGGER IF EXISTS clips_au;

-- Helper: the same chained REPLACE is applied at INSERT, DELETE, UPDATE, and
-- during the rebuild below. Lowercase'ing happens last so we don't have to
-- duplicate uppercase Turkish letters in the table — but `lower()` in SQLite
-- is ASCII-only by default, so we do explicit per-letter case folding for
-- the Turkish set BEFORE `lower()` handles the rest.
--
-- Order of REPLACEs is alphabetical by lowercase target for clarity; the
-- uppercase variant is folded immediately after its lowercase pair.

CREATE TRIGGER clips_ai AFTER INSERT ON clips
    WHEN new.text_content IS NOT NULL BEGIN
        INSERT INTO clips_fts(rowid, text_content) VALUES (
            new.rowid,
            lower(
                replace(replace(replace(replace(replace(replace(
                replace(replace(replace(replace(replace(replace(
                    new.text_content,
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
    WHEN old.text_content IS NOT NULL BEGIN
        INSERT INTO clips_fts(clips_fts, rowid, text_content) VALUES('delete', old.rowid,
            lower(
                replace(replace(replace(replace(replace(replace(
                replace(replace(replace(replace(replace(replace(
                    old.text_content,
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
    WHEN old.text_content IS NOT NULL OR new.text_content IS NOT NULL BEGIN
        INSERT INTO clips_fts(clips_fts, rowid, text_content) VALUES('delete', old.rowid,
            lower(
                replace(replace(replace(replace(replace(replace(
                replace(replace(replace(replace(replace(replace(
                    old.text_content,
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
                    new.text_content,
                    'Ç', 'c'), 'ç', 'c'),
                    'Ğ', 'g'), 'ğ', 'g'),
                    'İ', 'i'), 'ı', 'i'),
                    'Ö', 'o'), 'ö', 'o'),
                    'Ş', 's'), 'ş', 's'),
                    'Ü', 'u'), 'ü', 'u')
            )
        );
    END;

-- Rebuild the FTS index for existing rows so they're searchable with the new
-- folded alphabet. Wipe everything in clips_fts then re-insert all live rows.
INSERT INTO clips_fts(clips_fts) VALUES('delete-all');

INSERT INTO clips_fts(rowid, text_content)
    SELECT rowid,
        lower(
            replace(replace(replace(replace(replace(replace(
            replace(replace(replace(replace(replace(replace(
                text_content,
                'Ç', 'c'), 'ç', 'c'),
                'Ğ', 'g'), 'ğ', 'g'),
                'İ', 'i'), 'ı', 'i'),
                'Ö', 'o'), 'ö', 'o'),
                'Ş', 's'), 'ş', 's'),
                'Ü', 'u'), 'ü', 'u')
        )
    FROM clips
    WHERE text_content IS NOT NULL AND deleted_at IS NULL;

UPDATE settings SET value = '2' WHERE key = 'schema_version';
