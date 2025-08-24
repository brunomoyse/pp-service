-- Create club tables system to replace tournament-specific tables
-- Physical tables belong to clubs and are assigned to tournaments

-- Club Tables - Physical tables at each club
CREATE TABLE club_tables (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    club_id         UUID NOT NULL REFERENCES clubs(id) ON DELETE CASCADE,
    table_number    INTEGER NOT NULL,
    max_seats       INTEGER NOT NULL DEFAULT 9, -- Standard poker table seats
    table_name      TEXT, -- Optional custom name like "VIP Table", "Final Table"
    location        TEXT, -- Optional location description "Near bar", "Corner table"
    is_active       BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Ensure unique table numbers per club
    UNIQUE(club_id, table_number)
);

-- Tournament Table Assignments - Which club tables are used in which tournaments
CREATE TABLE tournament_table_assignments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id   UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    club_table_id   UUID NOT NULL REFERENCES club_tables(id) ON DELETE CASCADE,
    is_active       BOOLEAN NOT NULL DEFAULT true,
    assigned_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deactivated_at  TIMESTAMPTZ NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Prevent duplicate assignments of same table to same tournament
    UNIQUE(tournament_id, club_table_id)
);

-- Update existing table_seat_assignments to reference club_tables
-- We'll need to migrate existing data and update the foreign key
ALTER TABLE table_seat_assignments ADD COLUMN club_table_id UUID;

-- Create indexes for performance
CREATE INDEX club_tables_club_id_idx ON club_tables (club_id);
CREATE INDEX club_tables_is_active_idx ON club_tables (is_active);
CREATE INDEX tournament_table_assignments_tournament_id_idx ON tournament_table_assignments (tournament_id);
CREATE INDEX tournament_table_assignments_club_table_id_idx ON tournament_table_assignments (club_table_id);
CREATE INDEX tournament_table_assignments_is_active_idx ON tournament_table_assignments (is_active);

-- Note: updated_at columns are handled manually in the repository code
-- to maintain consistency with existing table patterns

-- Function to get available tables for a club (not currently assigned to active tournaments)
CREATE OR REPLACE FUNCTION get_available_club_tables(p_club_id UUID)
RETURNS TABLE (
    table_id UUID,
    table_number INTEGER,
    max_seats INTEGER,
    table_name TEXT,
    location TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        ct.id,
        ct.table_number,
        ct.max_seats,
        ct.table_name,
        ct.location
    FROM club_tables ct
    LEFT JOIN tournament_table_assignments tta ON ct.id = tta.club_table_id 
        AND tta.is_active = true
        AND EXISTS (
            SELECT 1 FROM tournaments t 
            WHERE t.id = tta.tournament_id 
            AND t.live_status IN ('not_started', 'late_registration', 'in_progress', 'break')
        )
    WHERE ct.club_id = p_club_id
        AND ct.is_active = true
        AND tta.id IS NULL -- Table not assigned to any active tournament
    ORDER BY ct.table_number;
END;
$$ LANGUAGE plpgsql;