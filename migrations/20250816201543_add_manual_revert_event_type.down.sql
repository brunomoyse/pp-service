-- Remove manual_revert from allowed event types for tournament clock events
ALTER TABLE tournament_clock_events 
DROP CONSTRAINT tournament_clock_events_event_type_check;

ALTER TABLE tournament_clock_events 
ADD CONSTRAINT tournament_clock_events_event_type_check 
CHECK (event_type IN ('start', 'pause', 'resume', 'level_advance', 'manual_advance', 'stop', 'reset'));