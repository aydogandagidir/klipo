-- 004_drop_tags.sql
-- The free-text tags feature from 003 is removed: its real purpose — telling
-- Klipo "what this clip is" (url / api key / phone / color / email / code) —
-- is already served by the auto-detected `category`, which is now made
-- user-editable instead (see commands::set_clip_category). Favoriting moves to
-- a dedicated star (backed by the existing `pinned` flag) rather than a tag.
--
-- So the tags + clip_tags tables become dead weight. Drop them. Dropping a
-- table also drops its indexes, so idx_clip_tags_* go with it.
--
-- Kept from 003: the `title` column, the `idx_clips_category` filter index, and
-- the FTS triggers that fold title+text_content (those don't reference tags).
--
-- IMPORTANT: never edit a shipped migration — add 005_* instead.

DROP TABLE IF EXISTS clip_tags;
DROP TABLE IF EXISTS tags;

UPDATE settings SET value = '4' WHERE key = 'schema_version';
