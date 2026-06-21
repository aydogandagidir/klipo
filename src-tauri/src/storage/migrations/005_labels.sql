-- 005_labels.sql
-- "Etiket" (label) system replaces the single, fixed `category`.
--
-- A clip can now carry MULTIPLE labels. Auto-detection still seeds the first
-- one (e.g. a URL gets "Bağlantı"), but the user can create new labels, rename
-- them (globally), and remove them. Labels are stored denormalized per clip:
--   - `name`     — the user-visible, renameable label text.
--   - `auto_key` — for auto-detected labels, the stable classifier key
--                  ('url','email',…) so detection survives a rename; NULL for
--                  user-created labels. Also drives the chip color in the UI.
--
-- The old `category` column is left in place (dead) on purpose: dropping a
-- column rewrites the whole table, and there's no functional need — nothing
-- reads it once 005 has migrated its values into clip_labels.
--
-- IMPORTANT: never edit a shipped migration — add 006_* instead.

CREATE TABLE clip_labels (
    clip_id  TEXT NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
    name     TEXT NOT NULL,
    auto_key TEXT,                 -- 'url','email',… for auto labels; NULL custom
    added_at INTEGER NOT NULL,
    PRIMARY KEY (clip_id, name)
);

CREATE INDEX idx_clip_labels_name ON clip_labels(name);
CREATE INDEX idx_clip_labels_clip ON clip_labels(clip_id);

-- Migrate existing auto categories into seed labels. The display names here
-- MUST match `clipboard::classify::auto_label_name` (used for new captures).
INSERT OR IGNORE INTO clip_labels (clip_id, name, auto_key, added_at)
SELECT
    id,
    CASE category
        WHEN 'url'    THEN 'Bağlantı'
        WHEN 'email'  THEN 'E-posta'
        WHEN 'phone'  THEN 'Telefon'
        WHEN 'iban'   THEN 'IBAN'
        WHEN 'color'  THEN 'Renk'
        WHEN 'code'   THEN 'Kod'
        WHEN 'json'   THEN 'JSON'
        WHEN 'number' THEN 'Sayı'
        WHEN 'path'   THEN 'Yol'
    END,
    category,
    created_at
FROM clips
WHERE deleted_at IS NULL
  AND category IN ('url','email','phone','iban','color','code','json','number','path');

-- The category filter index is obsolete now that labels drive filtering.
DROP INDEX IF EXISTS idx_clips_category;

UPDATE settings SET value = '5' WHERE key = 'schema_version';
