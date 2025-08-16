-- Drop functions
DROP FUNCTION IF EXISTS get_manager_clubs(UUID);
DROP FUNCTION IF EXISTS is_club_manager(UUID, UUID);

-- Drop trigger
DROP TRIGGER IF EXISTS trg_club_managers_updated_at ON club_managers;

-- Drop indexes
DROP INDEX IF EXISTS club_managers_is_active_idx;
DROP INDEX IF EXISTS club_managers_user_id_idx;
DROP INDEX IF EXISTS club_managers_club_id_idx;

-- Drop table
DROP TABLE IF EXISTS club_managers;