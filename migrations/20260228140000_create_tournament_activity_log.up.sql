-- Create unified tournament activity log table
CREATE TABLE tournament_activity_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    event_category TEXT NOT NULL CHECK (event_category IN (
        'clock', 'registration', 'seating', 'entry', 'result', 'tournament'
    )),
    event_action TEXT NOT NULL,
    actor_id UUID REFERENCES users(id),
    subject_id UUID REFERENCES users(id),
    event_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_activity_log_tournament_time ON tournament_activity_log(tournament_id, event_time DESC);

-- Append-only enforcement
CREATE RULE activity_log_no_update AS ON UPDATE TO tournament_activity_log DO INSTEAD NOTHING;
CREATE RULE activity_log_no_delete AS ON DELETE TO tournament_activity_log DO INSTEAD NOTHING;

-- Migrate existing clock events into the new table
INSERT INTO tournament_activity_log (id, tournament_id, event_category, event_action, actor_id, event_time, metadata)
SELECT id, tournament_id, 'clock', event_type, manager_id, event_time,
       CASE WHEN level_number IS NOT NULL
            THEN metadata || jsonb_build_object('level_number', level_number)
            ELSE metadata
       END
FROM tournament_clock_events;

-- Drop the old table
DROP TABLE tournament_clock_events;
