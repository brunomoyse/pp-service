-- Multi-day / flights.
--
-- A "series" is the event (e.g. a weekend Main Event). Each Day 1 *flight* and
-- the *final day* are ordinary `tournaments` rows linked by `series_id`, so the
-- existing clock / seating / entries / check-in / cash-report machinery works
-- per flight unchanged. Survivors of a flight produce a `flight_qualification`
-- (best stack forward); the final day's prize pool aggregates every flight's
-- entries (see the companion series-prize-pool migration), and results/points
-- are recorded once, on the final day.

-- 1. The event grouping.
CREATE TABLE tournament_series (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    club_id             UUID NOT NULL REFERENCES clubs(id) ON DELETE CASCADE,
    title               TEXT NOT NULL,
    -- A player surviving more than one flight carries their largest stack to
    -- Day 2 (the v1 behaviour). Reserved for future "most recent" / "no re-entry".
    best_stack_forward  BOOLEAN NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_tournament_series_club ON tournament_series (club_id);

CREATE TRIGGER trg_tournament_series_updated_at
    BEFORE UPDATE ON tournament_series
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- 2. Flight survivors that qualify for the final day. One row per
-- (series, player, source flight); `is_best` marks the stack carried to Day 2
-- when a player qualifies from more than one flight.
CREATE TABLE flight_qualifications (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    series_id           UUID NOT NULL REFERENCES tournament_series(id) ON DELETE CASCADE,
    club_player_id      UUID NOT NULL REFERENCES club_player(id),
    from_tournament_id  UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    chip_count          INTEGER NOT NULL,
    is_best             BOOLEAN NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (series_id, club_player_id, from_tournament_id)
);
CREATE INDEX idx_flight_qualifications_series ON flight_qualifications (series_id);

-- 3. Link tournaments to a series. NULL = a normal single-day tournament.
ALTER TABLE tournaments
    ADD COLUMN series_id     UUID REFERENCES tournament_series(id) ON DELETE SET NULL,
    ADD COLUMN flight_label  TEXT,
    ADD COLUMN is_final_day  BOOLEAN NOT NULL DEFAULT FALSE;
CREATE INDEX idx_tournaments_series ON tournaments (series_id);

-- 4. Starting stack carried into a tournament (imported Day 2 stack for a
-- qualifier). NULL = use the default starting stack.
ALTER TABLE tournament_registrations
    ADD COLUMN starting_stack INTEGER;

-- 5. When a seat assignment is created without an explicit stack, default it to
-- the player's registration starting_stack (the carried-over Day 2 stack). For
-- single-day tournaments starting_stack is always NULL, so this is a no-op.
CREATE OR REPLACE FUNCTION default_seat_stack_from_registration()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.stack_size IS NULL THEN
        SELECT starting_stack INTO NEW.stack_size
        FROM tournament_registrations
        WHERE tournament_id = NEW.tournament_id
          AND club_player_id = NEW.club_player_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_default_seat_stack
    BEFORE INSERT ON table_seat_assignments
    FOR EACH ROW EXECUTE FUNCTION default_seat_stack_from_registration();
