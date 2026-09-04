-- Club-level role hierarchy. Until now every club_managers row carried the same
-- authority: any co-manager could invite and revoke any other, including the
-- person who created the club. Two tiers now:
--   owner   -- manages the team (invite / revoke / set role) and the club's plan
--   manager -- everything operational: tournaments, clock, players, seating,
--              tables, templates, reports. Sees the team, cannot change it.
-- A club may have several owners, but never zero: the last one cannot be
-- demoted or removed (enforced in the API, alongside the last-manager guard).
ALTER TABLE club_managers
    ADD COLUMN role TEXT NOT NULL DEFAULT 'manager'
        CHECK (role IN ('owner', 'manager'));

-- Backfill: every existing club's founder becomes its owner. onboard_club
-- inserts the founding row with assigned_by NULL, while invites always set it,
-- so that flag identifies the founder. Fall back to the earliest assignment
-- (then id, for determinism) so a club whose founder was later revoked still
-- ends up with exactly one owner rather than none.
WITH founders AS (
    SELECT DISTINCT ON (club_id) id
    FROM club_managers
    WHERE is_active = true
    ORDER BY club_id, (assigned_by IS NULL) DESC, assigned_at, id
)
UPDATE club_managers cm
SET role = 'owner'
FROM founders f
WHERE cm.id = f.id;

-- Supports the "is there another owner left?" count guarding demote and revoke.
CREATE INDEX club_managers_owner_idx ON club_managers (club_id)
    WHERE role = 'owner' AND is_active = true;
