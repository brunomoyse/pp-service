-- Tables module: let a club predefine its physical tables and mark which ones
-- belong to the "default set" that is auto-linked to every new tournament.
ALTER TABLE club_tables
    ADD COLUMN is_default BOOLEAN NOT NULL DEFAULT true;

-- Existing tables are part of the default set (preserves prior behaviour where
-- the whole roster of tables was available to a tournament).
CREATE INDEX club_tables_default_idx ON club_tables (club_id, is_default)
    WHERE is_active;
