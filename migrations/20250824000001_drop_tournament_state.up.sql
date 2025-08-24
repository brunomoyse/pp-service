-- Drop tournament_state table as it's redundant with tournament_clocks
DROP TABLE IF EXISTS tournament_state CASCADE;

-- Note: All tournament timing/level data is now managed by:
-- - tournament_clocks: current level, timing state
-- - tournament_structures: level definitions (blinds, antes, durations)
-- - tournaments: core data including live_status and break_until