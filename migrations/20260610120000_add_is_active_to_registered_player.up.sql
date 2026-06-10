-- Soft-delete for roster entries. Managers archive a player instead of deleting:
-- operational tables (registrations/results/entries/seats) hold a NOT NULL
-- registered_player_id with ON DELETE SET NULL, so a hard delete of an entry
-- with history would fail. `is_active = false` hides the entry from the roster
-- view while preserving all references.
ALTER TABLE registered_player
    ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;

CREATE INDEX registered_player_is_active_idx ON registered_player (club_id, is_active);
