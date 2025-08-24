-- Drop the old tournament_tables system now that we use club tables with assignments

-- Since we're transitioning from tournament_tables to club_tables system,
-- and there shouldn't be critical seat assignment data in a development environment,
-- we'll clear existing seat assignments for a clean transition

-- Clear existing seat assignments to avoid constraint issues
DELETE FROM table_seat_assignments;

-- Make club_table_id NOT NULL since it's now the primary reference
ALTER TABLE table_seat_assignments ALTER COLUMN club_table_id SET NOT NULL;

-- Drop the foreign key constraint on tournament_tables
ALTER TABLE table_seat_assignments DROP CONSTRAINT IF EXISTS table_seat_assignments_table_id_fkey;

-- Drop the old table_id column from table_seat_assignments
ALTER TABLE table_seat_assignments DROP COLUMN IF EXISTS table_id;

-- Add foreign key constraint for club_table_id
ALTER TABLE table_seat_assignments 
ADD CONSTRAINT table_seat_assignments_club_table_id_fkey 
FOREIGN KEY (club_table_id) REFERENCES club_tables(id) ON DELETE CASCADE;

-- Create index on club_table_id (replacing the old table_id index)
DROP INDEX IF EXISTS table_seat_assignments_table_id_idx;
CREATE INDEX table_seat_assignments_club_table_id_idx ON table_seat_assignments (club_table_id);

-- Drop the old tournament_tables table and its dependencies
DROP INDEX IF EXISTS tournament_tables_is_active_idx;
DROP INDEX IF EXISTS tournament_tables_tournament_id_idx;
DROP TABLE IF EXISTS tournament_tables;