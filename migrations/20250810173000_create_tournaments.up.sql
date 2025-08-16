CREATE TABLE tournaments (
     id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
     club_id      UUID NOT NULL REFERENCES clubs(id) ON DELETE CASCADE,
     name         TEXT NOT NULL,
     description  TEXT,
     start_time   TIMESTAMPTZ NOT NULL,
     end_time     TIMESTAMPTZ,
     buy_in_cents INTEGER NOT NULL DEFAULT 0,     -- store money as integer cents
     seat_cap     INTEGER,                        -- optional max seats
     created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
     updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Useful indexes
CREATE INDEX tournaments_club_id_idx   ON tournaments (club_id);
CREATE INDEX tournaments_start_time_idx ON tournaments (start_time);
CREATE INDEX tournaments_name_trgm_idx ON tournaments USING GIN (LOWER(name) gin_trgm_ops);

-- updated_at trigger
CREATE TRIGGER trg_tournaments_updated_at
    BEFORE UPDATE ON tournaments
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();