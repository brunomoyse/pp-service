-- Rollback club tables system

-- Drop function
DROP FUNCTION IF EXISTS get_available_club_tables(UUID);

-- No triggers to drop (updated_at handled manually)

-- Drop indexes
DROP INDEX IF EXISTS tournament_table_assignments_is_active_idx;
DROP INDEX IF EXISTS tournament_table_assignments_club_table_id_idx;
DROP INDEX IF EXISTS tournament_table_assignments_tournament_id_idx;
DROP INDEX IF EXISTS club_tables_is_active_idx;
DROP INDEX IF EXISTS club_tables_club_id_idx;

-- Remove the added column from table_seat_assignments
ALTER TABLE table_seat_assignments DROP COLUMN IF EXISTS club_table_id;

-- Drop tables
DROP TABLE IF EXISTS tournament_table_assignments;
DROP TABLE IF EXISTS club_tables;