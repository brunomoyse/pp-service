-- Tournament Tables - represents physical tables in a tournament
CREATE TABLE tournament_tables (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id   UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    table_number    INTEGER NOT NULL,
    max_seats       INTEGER NOT NULL DEFAULT 9, -- Standard poker table seats
    is_active       BOOLEAN NOT NULL DEFAULT true,
    table_name      TEXT, -- Optional custom name like "Final Table"
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Ensure unique table numbers per tournament
    UNIQUE(tournament_id, table_number)
);

-- Table Seat Assignments - tracks current and historical seat assignments
CREATE TABLE table_seat_assignments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id   UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    table_id        UUID NOT NULL REFERENCES tournament_tables(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    seat_number     INTEGER NOT NULL CHECK (seat_number >= 1 AND seat_number <= 10),
    stack_size      INTEGER, -- Current chip count (nullable for when not tracked)
    is_current      BOOLEAN NOT NULL DEFAULT true, -- False for historical assignments
    assigned_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    unassigned_at   TIMESTAMPTZ, -- When player moved/eliminated
    assigned_by     UUID REFERENCES users(id), -- Who made the assignment (tournament director)
    notes           TEXT, -- Notes about the assignment/move
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Tournament Status Tracking - for live tournament state
CREATE TYPE tournament_live_status AS ENUM (
    'not_started',
    'registration_open', 
    'late_registration',
    'in_progress',
    'break',
    'final_table',
    'finished'
);

-- Add live status to tournaments table
ALTER TABLE tournaments 
ADD COLUMN live_status tournament_live_status NOT NULL DEFAULT 'not_started',
ADD COLUMN current_level INTEGER DEFAULT 1,
ADD COLUMN break_until TIMESTAMPTZ,
ADD COLUMN players_remaining INTEGER;

-- Useful indexes
CREATE INDEX tournament_tables_tournament_id_idx ON tournament_tables (tournament_id);
CREATE INDEX tournament_tables_is_active_idx ON tournament_tables (is_active);

CREATE INDEX table_seat_assignments_tournament_id_idx ON table_seat_assignments (tournament_id);
CREATE INDEX table_seat_assignments_table_id_idx ON table_seat_assignments (table_id);
CREATE INDEX table_seat_assignments_user_id_idx ON table_seat_assignments (user_id);
CREATE INDEX table_seat_assignments_is_current_idx ON table_seat_assignments (is_current);
CREATE INDEX table_seat_assignments_assigned_at_idx ON table_seat_assignments (assigned_at);

-- Ensure only one current assignment per seat per table
CREATE UNIQUE INDEX table_seat_assignments_unique_current_seat ON table_seat_assignments (table_id, seat_number) WHERE is_current = true;
-- Ensure a player can only be at one current seat
CREATE UNIQUE INDEX table_seat_assignments_unique_current_player ON table_seat_assignments (tournament_id, user_id) WHERE is_current = true;

-- Tournament live status index
CREATE INDEX tournaments_live_status_idx ON tournaments (live_status);

-- Updated at triggers
CREATE TRIGGER trg_tournament_tables_updated_at
    BEFORE UPDATE ON tournament_tables
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

CREATE TRIGGER trg_table_seat_assignments_updated_at
    BEFORE UPDATE ON table_seat_assignments
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- Function to automatically unassign old seats when creating new assignments
CREATE OR REPLACE FUNCTION handle_seat_assignment()
RETURNS TRIGGER AS $$
BEGIN
    -- If this is a current assignment, unassign any previous current assignment for this user
    IF NEW.is_current = true THEN
        UPDATE table_seat_assignments 
        SET is_current = false, 
            unassigned_at = NOW(),
            updated_at = NOW()
        WHERE tournament_id = NEW.tournament_id 
          AND user_id = NEW.user_id 
          AND is_current = true 
          AND id != NEW.id;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_handle_seat_assignment
    AFTER INSERT OR UPDATE ON table_seat_assignments
    FOR EACH ROW EXECUTE PROCEDURE handle_seat_assignment();