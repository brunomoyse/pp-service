-- Recreate tournament_tables system (rollback)

-- Recreate the tournament_tables table
CREATE TABLE tournament_tables (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id   UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    table_number    INTEGER NOT NULL,
    max_seats       INTEGER NOT NULL DEFAULT 9,
    is_active       BOOLEAN NOT NULL DEFAULT true,
    table_name      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(tournament_id, table_number)
);

-- Recreate indexes
CREATE INDEX tournament_tables_tournament_id_idx ON tournament_tables (tournament_id);
CREATE INDEX tournament_tables_is_active_idx ON tournament_tables (is_active);

-- Drop the club_table_id constraint and index from table_seat_assignments
DROP INDEX IF EXISTS table_seat_assignments_club_table_id_idx;
ALTER TABLE table_seat_assignments DROP CONSTRAINT IF EXISTS table_seat_assignments_club_table_id_fkey;

-- Add back the table_id column
ALTER TABLE table_seat_assignments ADD COLUMN table_id UUID;

-- Note: Data migration back would require recreating tournament_tables records
-- and updating table_seat_assignments.table_id accordingly
-- This is a destructive rollback that would lose table assignment data

-- Add back the foreign key constraint
ALTER TABLE table_seat_assignments 
ADD CONSTRAINT table_seat_assignments_table_id_fkey 
FOREIGN KEY (table_id) REFERENCES tournament_tables(id) ON DELETE CASCADE;

-- Remove club_table_id column (this will lose data!)
ALTER TABLE table_seat_assignments DROP COLUMN IF EXISTS club_table_id;