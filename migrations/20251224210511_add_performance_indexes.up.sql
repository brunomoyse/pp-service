-- Performance indexes for optimization

-- Clock service polling (runs every 5 seconds)
-- Optimizes: get_tournaments_to_advance() query
CREATE INDEX idx_tournament_clocks_auto_advance_poll
ON tournament_clocks (clock_status, auto_advance, level_end_time)
WHERE clock_status = 'running' AND auto_advance = true;

-- Upcoming tournaments query (runs every 60 seconds in notification service)
-- Optimizes: get_tournaments_starting_soon() query
CREATE INDEX idx_tournaments_starting_soon
ON tournaments (live_status, start_time)
WHERE live_status IN ('not_started', 'registration_open');

-- Occupied seats query (for batch seat availability check)
-- Optimizes: get_occupied_seats() query
CREATE INDEX idx_seat_assignments_occupied
ON table_seat_assignments (club_table_id, seat_number)
WHERE is_current = true;
