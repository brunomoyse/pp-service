-- Clear existing data (except clubs as requested)
DELETE FROM club_managers;
DELETE FROM table_seat_assignments;
DELETE FROM tournament_tables;
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
INSERT INTO users (id, email, username, first_name, last_name, phone, role, is_active) VALUES
    -- Club Managers
    ('ffffffff-ffff-ffff-ffff-ffffffffffff', 'manager1@phenix.be', null, 'Jean', 'Dupont', '+32477123456', 'manager', true),
    ('eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee', 'manager2@family.be', null, 'Marie', 'Martin', '+32477234567', 'manager', true),
    ('10101010-1010-1010-1010-101010101010', 'manager3@liege.be', null, 'Pierre', 'Leroy', '+32477345678', 'manager', true),
    ('20202020-2020-2020-2020-202020202020', 'manager4@brussels.be', 'brussels_mgr', 'Sophie', 'Bernard', '+32477456789', 'manager', true),
    -- Global Admin
    ('f0f0f0f0-f0f0-f0f0-f0f0-f0f0f0f0f0f0', 'admin@pocketpair.be', 'super_admin', 'Admin', 'Global', '+32477999999', 'admin', true),
    -- Belgian Players (using specific Belgian poker player names)
    ('30303030-3030-3030-3030-303030303030', 'damien@email.com', 'ace_killer', 'Damien', 'Hupé', '+32477567890', 'player', true),
    ('40404040-4040-4040-4040-404040404040', 'rico@email.com', 'poker_king', 'Rico', 'Chevalot', '+32477678901', 'player', true),
    ('50505050-5050-5050-5050-505050505050', 'aliosha@email.com', 'bluff_master', 'Aliosha', 'Staes', '+32477789012', 'player', true),
    ('60606060-6060-6060-6060-606060606060', 'rami@email.com', 'lucky_rami', 'Rami', 'Awad', '+32477890123', 'player', true),
    ('70707070-7070-7070-7070-707070707070', 'jeanmarie@email.com', 'card_shark', 'Jean-Marie', 'Vandeborne', '+32477901234', 'player', true),
    ('80808080-8080-8080-8080-808080808080', 'guillaume@email.com', 'river_master', 'Guillaume', 'Gillet', '+32478012345', 'player', true),
    ('90909090-9090-9090-9090-909090909090', 'manu@email.com', 'all_in_manu', 'Manu', 'Lecomte', '+32478123456', 'player', true),
    ('a0a0a0a0-a0a0-a0a0-a0a0-a0a0a0a0a0a0', 'fabien@email.com', 'tight_tiger', 'Fabien', 'Perrot', '+32478234567', 'player', true),
    ('b0b0b0b0-b0b0-b0b0-b0b0-b0b0b0b0b0b0', 'luca@email.com', 'fold_expert', 'Luca', 'Pecoraro', '+32478345678', 'player', true),
    ('c0c0c0c0-c0c0-c0c0-c0c0-c0c0c0c0c0c0', 'david@email.com', 'nuts_hunter', 'David', 'Opdebeek', '+32478456789', 'player', true),
    ('d0d0d0d0-d0d0-d0d0-d0d0-d0d0d0d0d0d0', 'danny@email.com', 'pocket_rockets', 'Danny', 'Covyn', '+32478567890', 'player', true),
    ('e0e0e0e0-e0e0-e0e0-e0e0-e0e0e0e0e0e0', 'sebastien@email.com', 'LE BANQUIER', 'Sébastien', 'Hetzel', '+32478678901', 'player', true);

-- Assign managers to clubs
INSERT INTO club_managers (id, club_id, user_id, assigned_by, notes) VALUES
    (gen_random_uuid(), '66666666-6666-6666-6666-666666666666', 'ffffffff-ffff-ffff-ffff-ffffffffffff', null, 'Manager of Poker One'),
    (gen_random_uuid(), 'cccccccc-cccc-cccc-cccc-cccccccccccc', '10101010-1010-1010-1010-101010101010', null, 'Manager of Liège Poker Club'),
    (gen_random_uuid(), 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee', null, 'Manager of Pokah Room Antwerp');

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
    ('10001111-1111-1111-1111-111111111111', '66666666-6666-6666-6666-666666666666', 'Monday Night Madness', 'Weekly freezeout', NOW() - INTERVAL '10 days', NOW() - INTERVAL '9 days', 2500, 40, 'finished'::tournament_live_status),
    ('10002222-2222-2222-2222-222222222222', '66666666-6666-6666-6666-666666666666', 'Tuesday Turbo', 'Fast-paced action', NOW() - INTERVAL '8 days', NOW() - INTERVAL '7 days', 3000, 30, 'finished'::tournament_live_status),
    ('10003333-3333-3333-3333-333333333333', '66666666-6666-6666-6666-666666666666', 'Wednesday Warriors', 'Mid-week grind', NOW() - INTERVAL '5 days', NOW() - INTERVAL '4 days', 4000, 50, 'finished'::tournament_live_status),
    -- 1 Live tournament
    ('10004444-4444-4444-4444-444444444444', '66666666-6666-6666-6666-666666666666', 'Thursday Live Event', 'Currently running', NOW() - INTERVAL '2 hours', null, 3500, 35,  'in_progress'::tournament_live_status),
    -- 3 Upcoming tournaments
    ('10005555-5555-5555-5555-555555555555', '66666666-6666-6666-6666-666666666666', 'Friday Fortune', 'Weekend starter', NOW() + INTERVAL '2 days', null, 4500, 45, 'not_started'::tournament_live_status),
    ('10006666-6666-6666-6666-666666666666', '66666666-6666-6666-6666-666666666666', 'Saturday Slam', 'Big weekend event', NOW() + INTERVAL '5 days', null, 5000, 60, 'not_started'::tournament_live_status),
    ('10007777-7777-7777-7777-777777777777', '66666666-6666-6666-6666-666666666666', 'Sunday Special', 'Weekly finale', NOW() + INTERVAL '8 days', null, 2000, 40, 'not_started'::tournament_live_status),

    -- Liège Poker Club - 7 tournaments
    -- 3 Completed tournaments
    ('20011111-1111-1111-1111-111111111111', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Liège Monday Classic', 'Classic format', NOW() - INTERVAL '12 days', NOW() - INTERVAL '11 days', 3000, 40,   'finished'::tournament_live_status),
    ('20012222-2222-2222-2222-222222222222', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Tuesday Deepstack', 'Long structure', NOW() - INTERVAL '9 days', NOW() - INTERVAL '8 days', 4000, 30,   'finished'::tournament_live_status),
    ('20013333-3333-3333-3333-333333333333', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Wednesday Rebuy', 'Rebuy allowed', NOW() - INTERVAL '6 days', NOW() - INTERVAL '5 days', 2500, 50,   'finished'::tournament_live_status),
    -- 1 Live tournament
    ('20014444-4444-4444-4444-444444444444', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Thursday Live Action', 'Currently playing', NOW() - INTERVAL '1 hour', null, 4500, 25,  'in_progress'::tournament_live_status),
    -- 3 Upcoming tournaments
    ('20015555-5555-5555-5555-555555555555', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Friday Night Fever', 'Popular weekly', NOW() + INTERVAL '3 days', null, 5000, 45,   'not_started'::tournament_live_status),
    ('20016666-6666-6666-6666-666666666666', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Saturday Superstack', 'Deep stacks', NOW() + INTERVAL '6 days', null, 3500, 60,   'not_started'::tournament_live_status),
    ('20017777-7777-7777-7777-777777777777', 'cccccccc-cccc-cccc-cccc-cccccccccccc', 'Sunday Series', 'Series event', NOW() + INTERVAL '9 days', null, 2000, 40,   'not_started'::tournament_live_status),

    -- Pokah Room Antwerp - 7 tournaments
    -- 3 Completed tournaments
    ('30021111-1111-1111-1111-111111111111', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Antwerp Ace', 'Monday special', NOW() - INTERVAL '11 days', NOW() - INTERVAL '10 days', 3500, 35,  'finished'::tournament_live_status),
    ('30022222-2222-2222-2222-222222222222', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Tuesday Tornado', 'Turbo format', NOW() - INTERVAL '7 days', NOW() - INTERVAL '6 days', 4500, 25,  'finished'::tournament_live_status),
    ('30023333-3333-3333-3333-333333333333', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Wednesday Wonder', 'Mid-week action', NOW() - INTERVAL '4 days', NOW() - INTERVAL '3 days', 3000, 45,  'finished'::tournament_live_status),
    -- 1 Live tournament
    ('30024444-4444-4444-4444-444444444444', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Thursday Thunder', 'High energy live', NOW() - INTERVAL '3 hours', null, 4000, 30, 'in_progress'::tournament_live_status),
    -- 3 Upcoming tournaments
    ('30025555-5555-5555-5555-555555555555', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Friday Fiesta', 'Party atmosphere', NOW() + INTERVAL '1 day', null, 3750, 50,  'not_started'::tournament_live_status),
    ('30026666-6666-6666-6666-666666666666', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Saturday Showdown', 'Weekend highlight', NOW() + INTERVAL '4 days', null, 5000, 70,  'not_started'::tournament_live_status),
    ('30027777-7777-7777-7777-777777777777', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'Sunday Summit', 'Weekly climax', NOW() + INTERVAL '7 days', null, 2500, 45,  'not_started'::tournament_live_status);

-- Register ALL players in ALL tournaments (12 players × 21 tournaments = 252 registrations)
INSERT INTO tournament_registrations (tournament_id, user_id, status)
SELECT t.id, u.id, 'registered'
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

-- Insert tournament state for live tournaments only
INSERT INTO tournament_state (tournament_id, current_level, players_remaining, current_small_blind, current_big_blind, current_ante, level_started_at, level_duration_minutes) VALUES
    -- Poker One - Thursday Live Event (Level 5)
    ('10004444-4444-4444-4444-444444444444', 5, 18, 200, 400, 50, NOW() - INTERVAL '15 minutes', 20),
    -- Liège - Thursday Live Action (Level 3)
    ('20014444-4444-4444-4444-444444444444', 3, 24, 100, 200, 25, NOW() - INTERVAL '8 minutes', 20),
    -- Antwerp - Thursday Thunder (Level 2)
    ('30024444-4444-4444-4444-444444444444', 2, 22, 50, 100, 0, NOW() - INTERVAL '12 minutes', 20);

-- Link tournaments to tags randomly
WITH tournament_tag_pairs AS (
    SELECT t.id as tournament_id, tag.id as tag_id, ROW_NUMBER() OVER (ORDER BY t.id) as rn
    FROM tournaments t
    CROSS JOIN tags tag
    WHERE MOD(ABS(HASHTEXT(t.id::text)), 4) = MOD(ABS(HASHTEXT(tag.id::text)), 4) -- Random assignment using modular hash
)
INSERT INTO tournament_tags (tournament_id, tag_id)
SELECT tournament_id, tag_id FROM tournament_tag_pairs LIMIT 50; -- Add some random tags