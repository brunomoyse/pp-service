DROP INDEX IF EXISTS club_player_last_name_idx;
ALTER TABLE club_player
    DROP COLUMN IF EXISTS first_name,
    DROP COLUMN IF EXISTS last_name;
