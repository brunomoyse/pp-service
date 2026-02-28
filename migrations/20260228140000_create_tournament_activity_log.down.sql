-- Recreate the original tournament_clock_events table
CREATE TABLE tournament_clock_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL CHECK (event_type IN ('start', 'pause', 'resume', 'level_advance', 'manual_advance', 'manual_revert', 'stop', 'reset')),
    level_number INTEGER,
    manager_id UUID REFERENCES users(id),
    event_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'
);

-- Migrate clock events back from activity log
INSERT INTO tournament_clock_events (id, tournament_id, event_type, level_number, manager_id, event_time, metadata)
SELECT id, tournament_id, event_action,
       (metadata->>'level_number')::INTEGER,
       actor_id, event_time,
       metadata - 'level_number'
FROM tournament_activity_log
WHERE event_category = 'clock';

-- Drop rules, index, and table
DROP RULE IF EXISTS activity_log_no_update ON tournament_activity_log;
DROP RULE IF EXISTS activity_log_no_delete ON tournament_activity_log;
DROP INDEX IF EXISTS idx_activity_log_tournament_time;
DROP TABLE tournament_activity_log;
