-- Tournaments, tournament clocks, and tournament structures

-- 7 tournaments per club (21 total): 3 completed, 1 live, 3 upcoming
INSERT INTO tournaments (id, club_id, name, description, start_time, end_time, buy_in_cents, seat_cap, live_status) VALUES
    -- Poker One (Charleroi)
    -- 3 Completed
    ('10001111-1111-1111-1111-111111111111', '66666666-6666-6666-6666-666666666666', 'Monday Night Madness', 'Weekly freezeout', NOW() - INTERVAL '10 days', NOW() - INTERVAL '9 days', 2500, 40, 'finished'),
    ('10002222-2222-2222-2222-222222222222', '66666666-6666-6666-6666-666666666666', 'Tuesday Turbo', 'Fast-paced action', NOW() - INTERVAL '8 days', NOW() - INTERVAL '7 days', 3000, 30, 'finished'),
    ('10003333-3333-3333-3333-333333333333', '66666666-6666-6666-6666-666666666666', 'Wednesday Warriors', 'Mid-week grind', NOW() - INTERVAL '5 days', NOW() - INTERVAL '4 days', 4000, 50, 'finished'),
    -- 1 Live
    ('10004444-4444-4444-4444-444444444444', '66666666-6666-6666-6666-666666666666', 'Thursday Live Event', 'Currently running', NOW() - INTERVAL '2 hours', null, 3500, 35, 'in_progress'),
    -- 3 Upcoming
    ('10005555-5555-5555-5555-555555555555', '66666666-6666-6666-6666-666666666666', 'Friday Fortune', 'Weekend starter', NOW() + INTERVAL '2 days', null, 4500, 45, 'not_started'),
    ('10006666-6666-6666-6666-666666666666', '66666666-6666-6666-6666-666666666666', 'Saturday Slam', 'Big weekend event', NOW() + INTERVAL '5 days', null, 5000, 60, 'not_started'),
    ('10007777-7777-7777-7777-777777777777', '66666666-6666-6666-6666-666666666666', 'Sunday Special', 'Weekly finale', NOW() + INTERVAL '8 days', null, 2000, 40, 'not_started'),

    -- Liège Poker Club
    -- 3 Completed
    ('20011111-1111-1111-1111-111111111111', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Liège Monday Classic', 'Classic format', NOW() - INTERVAL '12 days', NOW() - INTERVAL '11 days', 3000, 40, 'finished'),
    ('20012222-2222-2222-2222-222222222222', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Tuesday Deepstack', 'Long structure', NOW() - INTERVAL '9 days', NOW() - INTERVAL '8 days', 4000, 30, 'finished'),
    ('20013333-3333-3333-3333-333333333333', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Wednesday Rebuy', 'Rebuy allowed', NOW() - INTERVAL '6 days', NOW() - INTERVAL '5 days', 2500, 50, 'finished'),
    -- 1 Live
    ('20014444-4444-4444-4444-444444444444', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Thursday Live Action', 'Currently playing', NOW() - INTERVAL '1 hour', null, 4500, 25, 'in_progress'),
    -- 3 Upcoming
    ('20015555-5555-5555-5555-555555555555', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Friday Night Fever', 'Popular weekly', NOW() + INTERVAL '3 days', null, 5000, 45, 'not_started'),
    ('20016666-6666-6666-6666-666666666666', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Saturday Superstack', 'Deep stacks', NOW() + INTERVAL '6 days', null, 3500, 60, 'not_started'),
    ('20017777-7777-7777-7777-777777777777', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Sunday Series', 'Series event', NOW() + INTERVAL '9 days', null, 2000, 40, 'not_started'),

    -- Pokah Room Antwerp
    -- 3 Completed
    ('30021111-1111-1111-1111-111111111111', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Antwerp Ace', 'Monday special', NOW() - INTERVAL '11 days', NOW() - INTERVAL '10 days', 3500, 35, 'finished'),
    ('30022222-2222-2222-2222-222222222222', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Tuesday Tornado', 'Turbo format', NOW() - INTERVAL '7 days', NOW() - INTERVAL '6 days', 4500, 25, 'finished'),
    ('30023333-3333-3333-3333-333333333333', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Wednesday Wonder', 'Mid-week action', NOW() - INTERVAL '4 days', NOW() - INTERVAL '3 days', 3000, 45, 'finished'),
    -- 1 Live
    ('30024444-4444-4444-4444-444444444444', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Thursday Thunder', 'High energy live', NOW() - INTERVAL '3 hours', null, 4000, 30, 'in_progress'),
    -- 3 Upcoming
    ('30025555-5555-5555-5555-555555555555', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Friday Fiesta', 'Party atmosphere', NOW() + INTERVAL '1 day', null, 3750, 50, 'not_started'),
    ('30026666-6666-6666-6666-666666666666', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Saturday Showdown', 'Weekend highlight', NOW() + INTERVAL '4 days', null, 5000, 70, 'not_started'),
    ('30027777-7777-7777-7777-777777777777', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Sunday Summit', 'Weekly climax', NOW() + INTERVAL '7 days', null, 2500, 45, 'not_started');

-- Create tournament clocks for all tournaments (trigger only applies to new ones)
INSERT INTO tournament_clocks (tournament_id, clock_status, current_level, auto_advance)
SELECT id, 'stopped', 1, true
FROM tournaments
WHERE id NOT IN (SELECT tournament_id FROM tournament_clocks);

-- Create basic tournament structures for all tournaments that don't have them
INSERT INTO tournament_structures (tournament_id, level_number, small_blind, big_blind, ante, duration_minutes, is_break, break_duration_minutes)
SELECT t.id, s.level_number, s.small_blind, s.big_blind, s.ante, s.duration_minutes, s.is_break, s.break_duration_minutes
FROM tournaments t
CROSS JOIN (
    VALUES
        (1, 25, 50, 0, 20, false, null),
        (2, 50, 100, 0, 20, false, null),
        (3, 75, 150, 25, 20, false, null),
        (4, 100, 200, 25, 20, false, null),
        (5, 150, 300, 50, 20, false, null),
        (6, 200, 400, 50, 20, false, null),
        (7, 300, 600, 75, 20, false, null),
        (8, 400, 800, 100, 20, false, null),
        (9, 500, 1000, 100, 20, false, null),
        (10, 600, 1200, 200, 20, false, null),
        (11, 0, 0, 0, 15, true, 15),
        (12, 800, 1600, 200, 20, false, null),
        (13, 1000, 2000, 300, 20, false, null),
        (14, 1500, 3000, 500, 20, false, null),
        (15, 2000, 4000, 500, 20, false, null)
) AS s(level_number, small_blind, big_blind, ante, duration_minutes, is_break, break_duration_minutes)
WHERE NOT EXISTS (
    SELECT 1 FROM tournament_structures ts
    WHERE ts.tournament_id = t.id
);
