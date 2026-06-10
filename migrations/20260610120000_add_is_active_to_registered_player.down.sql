DROP INDEX IF EXISTS registered_player_is_active_idx;
ALTER TABLE registered_player DROP COLUMN is_active;
