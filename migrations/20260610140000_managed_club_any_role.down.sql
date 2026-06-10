-- Restore the role-gated definition: only users with role 'manager' surface a
-- managed club.
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
