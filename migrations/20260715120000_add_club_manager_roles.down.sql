DROP INDEX IF EXISTS club_managers_owner_idx;

ALTER TABLE club_managers
    DROP COLUMN IF EXISTS role;
