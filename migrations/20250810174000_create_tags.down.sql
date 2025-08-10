DROP INDEX IF EXISTS tournament_tags_tag_id_idx;
DROP TABLE IF EXISTS tournament_tags;

DROP TRIGGER IF EXISTS trg_tags_updated_at ON tags;
DROP INDEX IF EXISTS tags_label_trgm_idx;
DROP TABLE IF EXISTS tags;