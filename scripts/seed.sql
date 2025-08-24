-- Clear existing data (except clubs as requested)
DELETE FROM club_managers;
DELETE FROM table_seat_assignments;
-- tournament_tables no longer exists (removed in favor of club_tables system)
DELETE FROM tournament_results;
DELETE FROM tournament_registrations;
DELETE FROM player_deals;
DELETE FROM payout_templates;
DELETE FROM tournament_tags;
DELETE FROM tags;
DELETE FROM tournaments;
DELETE FROM users;

-- Recreate clubs to ensure we have consistent IDs
DELETE FROM clubs;

-- Insert clubs with known IDs for reference
INSERT INTO clubs (id, name, city, country) VALUES
    ('66666666-6666-6666-6666-666666666666', 'Poker One', 'Charleroi', 'BE'),
    ('cccccccc-cccc-cccc-cccc-cccccccccccc', 'Liège Poker Club', 'Liège', 'BE'),
    ('dddddddd-dddd-dddd-dddd-dddddddddddd', 'Pokah Room Antwerp', 'Antwerp', 'BE');

-- Insert users (mix of players and managers) using Belgian poker player names
INSERT INTO users (id, email, username, first_name, last_name, phone, role, is_active, password_hash) VALUES
    -- Club Managers
    ('ffffffff-ffff-ffff-ffff-ffffffffffff', 'contact@brunomoyse.be', null, 'Bruno', 'Moyse', '+32477123456', 'manager', true, '$2b$12$/5HEs8VNRQjb9olil8qV2.7FpVXROsv4ZRhqhG6qlhGiTG8A/GK86'),
    -- Global Admin
    ('f0f0f0f0-f0f0-f0f0-f0f0-f0f0f0f0f0f0', 'admin@pocketpair.be', 'super_admin', 'Admin', 'Global', '+32477999999', 'admin', true, '$2b$12$/5HEs8VNRQjb9olil8qV2.7FpVXROsv4ZRhqhG6qlhGiTG8A/GK86'),
    -- Belgian Players (using specific Belgian poker player names)
    ('30303030-3030-3030-3030-303030303030', 'damien@email.com', 'ace_killer', 'Damien', 'Hupé', '+32477567890', 'player', true, null),
    ('40404040-4040-4040-4040-404040404040', 'rico@email.com', 'poker_king', 'Rico', 'Chevalot', '+32477678901', 'player', true, null),
    ('50505050-5050-5050-5050-505050505050', 'aliosha@email.com', 'bluff_master', 'Aliosha', 'Staes', '+32477789012', 'player', true, null),
    ('60606060-6060-6060-6060-606060606060', 'rami@email.com', 'lucky_rami', 'Rami', 'Awad', '+32477890123', 'player', true, null),
    ('70707070-7070-7070-7070-707070707070', 'jeanmarie@email.com', 'card_shark', 'Jean-Marie', 'Vandeborne', '+32477901234', 'player', true, null),
    ('80808080-8080-8080-8080-808080808080', 'guillaume@email.com', 'river_master', 'Guillaume', 'Gillet', '+32478012345', 'player', true, null),
    ('90909090-9090-9090-9090-909090909090', 'manu@email.com', 'all_in_manu', 'Manu', 'Lecomte', '+32478123456', 'player', true, null),
    ('a0a0a0a0-a0a0-a0a0-a0a0-a0a0a0a0a0a0', 'fabien@email.com', 'tight_tiger', 'Fabien', 'Perrot', '+32478234567', 'player', true, null),
    ('b0b0b0b0-b0b0-b0b0-b0b0-b0b0b0b0b0b0', 'luca@email.com', 'fold_expert', 'Luca', 'Pecoraro', '+32478345678', 'player', true, null),
    ('c0c0c0c0-c0c0-c0c0-c0c0-c0c0c0c0c0c0', 'david@email.com', 'nuts_hunter', 'David', 'Opdebeek', '+32478456789', 'player', true, null),
    ('d0d0d0d0-d0d0-d0d0-d0d0-d0d0d0d0d0d0', 'danny@email.com', 'pocket_rockets', 'Danny', 'Covyn', '+32478567890', 'player', true, null),
    ('e0e0e0e0-e0e0-e0e0-e0e0-e0e0e0e0e0e0', 'sebastien@email.com', 'LE BANQUIER', 'Sébastien', 'Hetzel', '+32478678901', 'player', true, null);

-- Assign managers to clubs
INSERT INTO club_managers (id, club_id, user_id, assigned_by, notes) VALUES
    (gen_random_uuid(), '66666666-6666-6666-6666-666666666666', 'ffffffff-ffff-ffff-ffff-ffffffffffff', null, 'Manager of Poker One'),

-- Insert club tables (physical tables at each club)
INSERT INTO club_tables (id, club_id, table_number, max_seats) VALUES
    -- Poker One tables (Charleroi)
    ('11111111-1111-1111-1111-111111111111', '66666666-6666-6666-6666-666666666666', 1, 9),
    ('11111111-1111-1111-1111-111111111112', '66666666-6666-6666-6666-666666666666', 2, 9),
    ('11111111-1111-1111-1111-111111111113', '66666666-6666-6666-6666-666666666666', 3, 8),
    ('11111111-1111-1111-1111-111111111114', '66666666-6666-6666-6666-666666666666', 4, 6);

-- Insert tags
INSERT INTO tags (id, slug, label) VALUES
    (gen_random_uuid(), 'freezeout', 'Freezeout'),
    (gen_random_uuid(), 'rebuy', 'Rebuy'),
    (gen_random_uuid(), 'addon', 'Add-on'),
    (gen_random_uuid(), 'turbo', 'Turbo'),
    (gen_random_uuid(), 'deepstack', 'Deepstack'),
    (gen_random_uuid(), 'satellite', 'Satellite'),
    (gen_random_uuid(), 'freeroll', 'Freeroll'),
    (gen_random_uuid(), 'bounty', 'Bounty');

-- Insert payout templates
INSERT INTO payout_templates (id, name, description, min_players, max_players, payout_structure) VALUES
    (gen_random_uuid(), 'Standard 9-18 Players', 'Standard payout for small tournaments', 9, 18, 
     '{"payouts": [{"position": 1, "percentage": 50}, {"position": 2, "percentage": 30}, {"position": 3, "percentage": 20}]}'),
    (gen_random_uuid(), 'Big Tournament 50+', 'Payout structure for large tournaments', 50, null, 
     '{"payouts": [{"position": 1, "percentage": 40}, {"position": 2, "percentage": 25}, {"position": 3, "percentage": 15}, {"position": 4, "percentage": 10}, {"position": 5, "percentage": 6}, {"position": 6, "percentage": 4}]}');

-- Insert 7 tournaments per club (21 total tournaments): 3 upcoming, 1 live, 3 completed
INSERT INTO tournaments (id, club_id, name, description, start_time, end_time, buy_in_cents, seat_cap, live_status) VALUES
    -- Poker One (Charleroi) - 7 tournaments
    -- 3 Completed tournaments
    ('10001111-1111-1111-1111-111111111111', '66666666-6666-6666-6666-666666666666', 'Monday Night Madness', 'Weekly freezeout', NOW() - INTERVAL '10 days', NOW() - INTERVAL '9 days', 2500, 40, 'finished'),
    ('10002222-2222-2222-2222-222222222222', '66666666-6666-6666-6666-666666666666', 'Tuesday Turbo', 'Fast-paced action', NOW() - INTERVAL '8 days', NOW() - INTERVAL '7 days', 3000, 30, 'finished'),
    ('10003333-3333-3333-3333-333333333333', '66666666-6666-6666-6666-666666666666', 'Wednesday Warriors', 'Mid-week grind', NOW() - INTERVAL '5 days', NOW() - INTERVAL '4 days', 4000, 50, 'finished'),
    -- 1 Live tournament
    ('10004444-4444-4444-4444-444444444444', '66666666-6666-6666-6666-666666666666', 'Thursday Live Event', 'Currently running', NOW() - INTERVAL '2 hours', null, 3500, 35, 'in_progress'),
    -- 3 Upcoming tournaments
    ('10005555-5555-5555-5555-555555555555', '66666666-6666-6666-6666-666666666666', 'Friday Fortune', 'Weekend starter', NOW() + INTERVAL '2 days', null, 4500, 45, 'not_started'),
    ('10006666-6666-6666-6666-666666666666', '66666666-6666-6666-6666-666666666666', 'Saturday Slam', 'Big weekend event', NOW() + INTERVAL '5 days', null, 5000, 60, 'not_started'),
    ('10007777-7777-7777-7777-777777777777', '66666666-6666-6666-6666-666666666666', 'Sunday Special', 'Weekly finale', NOW() + INTERVAL '8 days', null, 2000, 40, 'not_started'),

    -- Liège Poker Club - 7 tournaments
    -- 3 Completed tournaments
    ('20011111-1111-1111-1111-111111111111', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Liège Monday Classic', 'Classic format', NOW() - INTERVAL '12 days', NOW() - INTERVAL '11 days', 3000, 40, 'finished'),
    ('20012222-2222-2222-2222-222222222222', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Tuesday Deepstack', 'Long structure', NOW() - INTERVAL '9 days', NOW() - INTERVAL '8 days', 4000, 30, 'finished'),
    ('20013333-3333-3333-3333-333333333333', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Wednesday Rebuy', 'Rebuy allowed', NOW() - INTERVAL '6 days', NOW() - INTERVAL '5 days', 2500, 50, 'finished'),
    -- 1 Live tournament
    ('20014444-4444-4444-4444-444444444444', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Thursday Live Action', 'Currently playing', NOW() - INTERVAL '1 hour', null, 4500, 25, 'in_progress'),
    -- 3 Upcoming tournaments
    ('20015555-5555-5555-5555-555555555555', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Friday Night Fever', 'Popular weekly', NOW() + INTERVAL '3 days', null, 5000, 45, 'not_started'),
    ('20016666-6666-6666-6666-666666666666', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Saturday Superstack', 'Deep stacks', NOW() + INTERVAL '6 days', null, 3500, 60, 'not_started'),
    ('20017777-7777-7777-7777-777777777777', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Sunday Series', 'Series event', NOW() + INTERVAL '9 days', null, 2000, 40, 'not_started'),

    -- Pokah Room Antwerp - 7 tournaments
    -- 3 Completed tournaments
    ('30021111-1111-1111-1111-111111111111', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Antwerp Ace', 'Monday special', NOW() - INTERVAL '11 days', NOW() - INTERVAL '10 days', 3500, 35, 'finished'),
    ('30022222-2222-2222-2222-222222222222', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Tuesday Tornado', 'Turbo format', NOW() - INTERVAL '7 days', NOW() - INTERVAL '6 days', 4500, 25, 'finished'),
    ('30023333-3333-3333-3333-333333333333', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Wednesday Wonder', 'Mid-week action', NOW() - INTERVAL '4 days', NOW() - INTERVAL '3 days', 3000, 45, 'finished'),
    -- 1 Live tournament
    ('30024444-4444-4444-4444-444444444444', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Thursday Thunder', 'High energy live', NOW() - INTERVAL '3 hours', null, 4000, 30, 'in_progress'),
    -- 3 Upcoming tournaments
    ('30025555-5555-5555-5555-555555555555', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Friday Fiesta', 'Party atmosphere', NOW() + INTERVAL '1 day', null, 3750, 50, 'not_started'),
    ('30026666-6666-6666-6666-666666666666', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Saturday Showdown', 'Weekend highlight', NOW() + INTERVAL '4 days', null, 5000, 70, 'not_started'),
    ('30027777-7777-7777-7777-777777777777', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Sunday Summit', 'Weekly climax', NOW() + INTERVAL '7 days', null, 2500, 45, 'not_started');

-- Register ALL players in ALL tournaments (12 players × 21 tournaments = 252 registrations)
INSERT INTO tournament_registrations (tournament_id, user_id, status)
SELECT t.id, u.id, 'pending'
FROM tournaments t
CROSS JOIN (
    SELECT id FROM users 
    WHERE role = 'player'
) u;

-- Generate tournament results for finished tournaments only (9 total: 3 per club)
-- Jean-Marie (70707070-7070-7070-7070-707070707070) will have the most wins and prize money

INSERT INTO tournament_results (tournament_id, user_id, final_position, prize_cents) VALUES
-- Poker One Tournaments
('10001111-1111-1111-1111-111111111111', '70707070-7070-7070-7070-707070707070', 1, 75000),  -- €750 (Monday Night Madness - €25 buy-in)
('10001111-1111-1111-1111-111111111111', '40404040-4040-4040-4040-404040404040', 2, 45000),  -- €450
('10001111-1111-1111-1111-111111111111', '50505050-5050-5050-5050-505050505050', 3, 30000),  -- €300

('10002222-2222-2222-2222-222222222222', '70707070-7070-7070-7070-707070707070', 1, 90000),  -- €900 (Tuesday Turbo - €30 buy-in)
('10002222-2222-2222-2222-222222222222', '30303030-3030-3030-3030-303030303030', 2, 54000),  -- €540
('10002222-2222-2222-2222-222222222222', '80808080-8080-8080-8080-808080808080', 3, 36000),  -- €360

('10003333-3333-3333-3333-333333333333', '70707070-7070-7070-7070-707070707070', 1, 120000), -- €1200 (Wednesday Warriors - €40 buy-in)
('10003333-3333-3333-3333-333333333333', '60606060-6060-6060-6060-606060606060', 2, 72000),  -- €720
('10003333-3333-3333-3333-333333333333', 'c0c0c0c0-c0c0-c0c0-c0c0-c0c0c0c0c0c0', 3, 48000),  -- €480

-- Liège Poker Club Tournaments
('20011111-1111-1111-1111-111111111111', '70707070-7070-7070-7070-707070707070', 1, 90000),  -- €900 (Liège Monday Classic - €30 buy-in)
('20011111-1111-1111-1111-111111111111', '30303030-3030-3030-3030-303030303030', 2, 54000),  -- €540
('20011111-1111-1111-1111-111111111111', '40404040-4040-4040-4040-404040404040', 3, 36000),  -- €360

('20012222-2222-2222-2222-222222222222', '70707070-7070-7070-7070-707070707070', 1, 120000), -- €1200 (Tuesday Deepstack - €40 buy-in)
('20012222-2222-2222-2222-222222222222', '50505050-5050-5050-5050-505050505050', 2, 72000),  -- €720
('20012222-2222-2222-2222-222222222222', '60606060-6060-6060-6060-606060606060', 3, 48000),  -- €480

('20013333-3333-3333-3333-333333333333', '70707070-7070-7070-7070-707070707070', 1, 75000),  -- €750 (Wednesday Rebuy - €25 buy-in)
('20013333-3333-3333-3333-333333333333', '80808080-8080-8080-8080-808080808080', 2, 45000),  -- €450
('20013333-3333-3333-3333-333333333333', '90909090-9090-9090-9090-909090909090', 3, 30000),  -- €300

-- Pokah Room Antwerp Tournaments
('30021111-1111-1111-1111-111111111111', '70707070-7070-7070-7070-707070707070', 1, 105000), -- €1050 (Antwerp Ace - €35 buy-in)
('30021111-1111-1111-1111-111111111111', 'b0b0b0b0-b0b0-b0b0-b0b0-b0b0b0b0b0b0', 2, 63000),  -- €630
('30021111-1111-1111-1111-111111111111', 'c0c0c0c0-c0c0-c0c0-c0c0-c0c0c0c0c0c0', 3, 42000),  -- €420

('30022222-2222-2222-2222-222222222222', '30303030-3030-3030-3030-303030303030', 1, 135000), -- €1350 (Tuesday Tornado - €45 buy-in)
('30022222-2222-2222-2222-222222222222', '70707070-7070-7070-7070-707070707070', 2, 81000),  -- €810
('30022222-2222-2222-2222-222222222222', 'e0e0e0e0-e0e0-e0e0-e0e0-e0e0e0e0e0e0', 3, 54000),  -- €540

('30023333-3333-3333-3333-333333333333', '70707070-7070-7070-7070-707070707070', 1, 90000),  -- €900 (Wednesday Wonder - €30 buy-in)
('30023333-3333-3333-3333-333333333333', 'd0d0d0d0-d0d0-d0d0-d0d0-d0d0d0d0d0d0', 2, 54000),  -- €540
('30023333-3333-3333-3333-333333333333', '40404040-4040-4040-4040-404040404040', 3, 36000);  -- €360

-- Link tournaments to tags randomly
WITH tournament_tag_pairs AS (
    SELECT t.id as tournament_id, tag.id as tag_id, ROW_NUMBER() OVER (ORDER BY t.id) as rn
    FROM tournaments t
    CROSS JOIN tags tag
    WHERE MOD(ABS(HASHTEXT(t.id::text)), 4) = MOD(ABS(HASHTEXT(tag.id::text)), 4) -- Random assignment using modular hash
)
INSERT INTO tournament_tags (tournament_id, tag_id)
SELECT tournament_id, tag_id FROM tournament_tag_pairs LIMIT 50; -- Add some random tags

-- Calculate points for all tournaments with results using the PostgreSQL function
SELECT recalculate_all_tournament_points();

-- Delete existing payout templates to avoid conflicts
DELETE FROM payout_templates;

-- Insert payout templates based on provided structure
INSERT INTO payout_templates (name, description, min_players, max_players, payout_structure)
VALUES 
    -- 2 players - Winner takes all (not in table, keeping standard)
    ('Heads Up', 'Winner takes all for 2 players', 2, 2, 
     '[{"position": 1, "percentage": 100.0}]'::jsonb),
    
    -- 3-10 players
    ('3-10 Players', 'Top 2 paid', 3, 10,
     '[{"position": 1, "percentage": 70.0}, {"position": 2, "percentage": 30.0}]'::jsonb),
    
    -- 11-20 players
    ('11-20 Players', 'Top 3 paid', 11, 20,
     '[{"position": 1, "percentage": 50.0}, {"position": 2, "percentage": 30.0}, {"position": 3, "percentage": 20.0}]'::jsonb),
    
    -- 21-30 players
    ('21-30 Players', 'Top 5 paid', 21, 30,
     '[{"position": 1, "percentage": 37.0}, {"position": 2, "percentage": 25.0}, {"position": 3, "percentage": 15.0}, {"position": 4, "percentage": 12.0}, {"position": 5, "percentage": 11.0}]'::jsonb),
    
    -- 31-40 players
    ('31-40 Players', 'Top 6 paid', 31, 40,
     '[{"position": 1, "percentage": 35.0}, {"position": 2, "percentage": 22.0}, {"position": 3, "percentage": 15.0}, {"position": 4, "percentage": 11.0}, {"position": 5, "percentage": 9.0}, {"position": 6, "percentage": 8.0}]'::jsonb),
    
    -- 41-50 players
    ('41-50 Players', 'Top 8 paid', 41, 50,
     '[{"position": 1, "percentage": 32.0}, {"position": 2, "percentage": 18.0}, {"position": 3, "percentage": 12.5}, {"position": 4, "percentage": 10.5}, {"position": 5, "percentage": 8.3}, {"position": 6, "percentage": 7.3}, {"position": 7, "percentage": 6.2}, {"position": 8, "percentage": 5.2}]'::jsonb),
    
    -- 51-60 players
    ('51-60 Players', 'Top 9 paid', 51, 60,
     '[{"position": 1, "percentage": 30.0}, {"position": 2, "percentage": 17.5}, {"position": 3, "percentage": 12.2}, {"position": 4, "percentage": 10.2}, {"position": 5, "percentage": 8.1}, {"position": 6, "percentage": 7.1}, {"position": 7, "percentage": 6.1}, {"position": 8, "percentage": 5.1}, {"position": 9, "percentage": 3.7}]'::jsonb),
    
    -- 61-75 players
    ('61-75 Players', 'Top 10 paid', 61, 75,
     '[{"position": 1, "percentage": 29.0}, {"position": 2, "percentage": 17.0}, {"position": 3, "percentage": 12.0}, {"position": 4, "percentage": 10.0}, {"position": 5, "percentage": 8.0}, {"position": 6, "percentage": 6.9}, {"position": 7, "percentage": 5.9}, {"position": 8, "percentage": 4.9}, {"position": 9, "percentage": 3.5}, {"position": 10, "percentage": 2.8}]'::jsonb),
    
    -- 76+ players (large tournaments)
    ('76+ Players', 'Top 10 paid', 76, NULL,
     '[{"position": 1, "percentage": 28.0}, {"position": 2, "percentage": 16.0}, {"position": 3, "percentage": 11.5}, {"position": 4, "percentage": 9.5}, {"position": 5, "percentage": 7.5}, {"position": 6, "percentage": 6.5}, {"position": 7, "percentage": 5.5}, {"position": 8, "percentage": 4.5}, {"position": 9, "percentage": 3.0}, {"position": 10, "percentage": 2.5}, {"position": 11, "percentage": 2.0}, {"position": 12, "percentage": 1.5}, {"position": 13, "percentage": 1.0}, {"position": 14, "percentage": 0.7}, {"position": 15, "percentage": 0.3}]'::jsonb)
ON CONFLICT DO NOTHING;

-- Create tournament clocks for all existing tournaments (since trigger only applies to new tournaments)
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
        (11, 0, 0, 0, 15, true, 15), -- Break level
        (12, 800, 1600, 200, 20, false, null),
        (13, 1000, 2000, 300, 20, false, null),
        (14, 1500, 3000, 500, 20, false, null),
        (15, 2000, 4000, 500, 20, false, null)
) AS s(level_number, small_blind, big_blind, ante, duration_minutes, is_break, break_duration_minutes)
WHERE NOT EXISTS (
    SELECT 1 FROM tournament_structures ts 
    WHERE ts.tournament_id = t.id
);