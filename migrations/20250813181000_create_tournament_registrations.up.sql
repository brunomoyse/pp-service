CREATE TABLE tournament_registrations (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id   UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    registration_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status          TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'waitlisted', 'cancelled', 'no_show')),
    notes           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Ensure a user can only register once per tournament
    UNIQUE(tournament_id, user_id)
);

-- Useful indexes
CREATE INDEX tournament_registrations_tournament_id_idx ON tournament_registrations (tournament_id);
CREATE INDEX tournament_registrations_user_id_idx ON tournament_registrations (user_id);
CREATE INDEX tournament_registrations_status_idx ON tournament_registrations (status);
CREATE INDEX tournament_registrations_registration_time_idx ON tournament_registrations (registration_time);

-- updated_at trigger
CREATE TRIGGER trg_tournament_registrations_updated_at
    BEFORE UPDATE ON tournament_registrations
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();