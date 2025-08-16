-- Drop indexes
DROP INDEX IF EXISTS idx_tournament_clocks_status;
DROP INDEX IF EXISTS idx_tournament_clocks_tournament_id;
DROP INDEX IF EXISTS idx_tournament_structures_level;
DROP INDEX IF EXISTS idx_tournament_structures_tournament_id;
DROP INDEX IF EXISTS idx_tournament_clock_events_time;
DROP INDEX IF EXISTS idx_tournament_clock_events_tournament_id;

-- Drop trigger
DROP TRIGGER IF EXISTS trg_tournament_clocks_updated_at ON tournament_clocks;

-- Drop tables
DROP TABLE IF EXISTS tournament_clock_events;
DROP TABLE IF EXISTS tournament_clocks;
DROP TABLE IF EXISTS tournament_structures;