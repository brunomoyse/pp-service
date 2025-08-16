-- Club Managers junction table - defines which managers can manage which clubs
CREATE TABLE club_managers (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    club_id         UUID NOT NULL REFERENCES clubs(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    assigned_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    assigned_by     UUID REFERENCES users(id), -- Who assigned this manager (admin/owner)
    is_active       BOOLEAN NOT NULL DEFAULT true,
    notes           TEXT, -- Optional notes about the assignment
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create a partial unique index to ensure a user can only be assigned once per club (as active manager)
CREATE UNIQUE INDEX club_managers_unique_active_assignment ON club_managers (club_id, user_id) WHERE is_active = true;

-- Useful indexes
CREATE INDEX club_managers_club_id_idx ON club_managers (club_id);
CREATE INDEX club_managers_user_id_idx ON club_managers (user_id);
CREATE INDEX club_managers_is_active_idx ON club_managers (is_active);

-- Updated at trigger
CREATE TRIGGER trg_club_managers_updated_at
    BEFORE UPDATE ON club_managers
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- Function to verify if a user is an active manager of a specific club
CREATE OR REPLACE FUNCTION is_club_manager(manager_user_id UUID, target_club_id UUID)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 
        FROM club_managers cm
        JOIN users u ON cm.user_id = u.id
        WHERE cm.user_id = manager_user_id 
          AND cm.club_id = target_club_id
          AND cm.is_active = true
          AND u.role = 'manager'
          AND u.is_active = true
    );
END;
$$ LANGUAGE plpgsql;

-- Function to get all clubs a manager can manage
CREATE OR REPLACE FUNCTION get_manager_clubs(manager_user_id UUID)
RETURNS TABLE(club_id UUID, club_name TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT c.id, c.name
    FROM clubs c
    JOIN club_managers cm ON c.id = cm.club_id
    JOIN users u ON cm.user_id = u.id
    WHERE cm.user_id = manager_user_id
      AND cm.is_active = true
      AND u.role = 'manager'
      AND u.is_active = true;
END;
$$ LANGUAGE plpgsql;