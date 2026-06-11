DROP TRIGGER IF EXISTS trg_default_seat_stack ON table_seat_assignments;
DROP FUNCTION IF EXISTS default_seat_stack_from_registration();

ALTER TABLE tournament_registrations DROP COLUMN IF EXISTS starting_stack;

DROP INDEX IF EXISTS idx_tournaments_series;
ALTER TABLE tournaments
    DROP COLUMN IF EXISTS series_id,
    DROP COLUMN IF EXISTS flight_label,
    DROP COLUMN IF EXISTS is_final_day;

DROP TABLE IF EXISTS flight_qualifications;

DROP TRIGGER IF EXISTS trg_tournament_series_updated_at ON tournament_series;
DROP TABLE IF EXISTS tournament_series;
