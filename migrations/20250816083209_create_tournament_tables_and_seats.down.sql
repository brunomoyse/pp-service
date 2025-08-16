-- Drop triggers
DROP TRIGGER IF EXISTS trg_handle_seat_assignment ON table_seat_assignments;
DROP TRIGGER IF EXISTS trg_table_seat_assignments_updated_at ON table_seat_assignments;
DROP TRIGGER IF EXISTS trg_tournament_tables_updated_at ON tournament_tables;

-- Drop function
DROP FUNCTION IF EXISTS handle_seat_assignment();

-- Drop indexes
DROP INDEX IF EXISTS tournaments_live_status_idx;
DROP INDEX IF EXISTS table_seat_assignments_assigned_at_idx;
DROP INDEX IF EXISTS table_seat_assignments_is_current_idx;
DROP INDEX IF EXISTS table_seat_assignments_user_id_idx;
DROP INDEX IF EXISTS table_seat_assignments_table_id_idx;
DROP INDEX IF EXISTS table_seat_assignments_tournament_id_idx;
DROP INDEX IF EXISTS tournament_tables_is_active_idx;
DROP INDEX IF EXISTS tournament_tables_tournament_id_idx;

-- Remove columns from tournaments table
ALTER TABLE tournaments 
DROP COLUMN IF EXISTS players_remaining,
DROP COLUMN IF EXISTS break_until,
DROP COLUMN IF EXISTS current_level,
DROP COLUMN IF EXISTS live_status;

-- Drop enum type
DROP TYPE IF EXISTS tournament_live_status;

-- Drop tables
DROP TABLE IF EXISTS table_seat_assignments;
DROP TABLE IF EXISTS tournament_tables;