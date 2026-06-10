-- Surface a user's club_managers assignments as their "managed club" regardless
-- of the user's role. An admin explicitly assigned to a club (via club_managers)
-- should see that club as their active context in the manager app. Authorization
-- stays role-based (admins manage any club, see require_club_manager) and is
-- unaffected — this function only powers the User.managedClub display field.
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
      AND u.is_active = true;
END;
$$ LANGUAGE plpgsql;
