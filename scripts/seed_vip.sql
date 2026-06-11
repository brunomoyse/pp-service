-- ============================================================================
-- VIP Poker Club — demo seed
-- ============================================================================
-- Idempotent: wipes all domain data (keeps the migration-seeded catalogs:
-- achievements, cosmetic_item, blind_structure_templates) and rebuilds one
-- believable world so a demo exercises every feature.
--
-- Run:  docker exec -i <postgres-container> psql -U <user> -d pocketpair < scripts/seed_vip.sql
--
-- Deterministic UUIDs are used throughout so the script is re-runnable and
-- cross-references are trivial:
--   club            11111111-...-111111111111
--   app users       a0000000-...-0000000000NN   (00 = Bruno/admin, 01..09 players)
--   roster (app)    b0000000-...-0000000000NN   (mirrors the app user index)
--   roster (no app) b0000000-...-0000000001NN   (NN = 01..31 account-less players)
--   tournaments     d0000000-...-0000000000NN
--   club tables     e0000000-...-0000000000NN
--   payout tmpl     f0000000-...-0000000000NN
-- The link_club_player() trigger stamps user_id from the roster, so child
-- rows (registrations/entries/results/seats) only ever supply club_player_id.
-- ============================================================================

BEGIN;

-- Admin password is the bcrypt hash of "admin" (inserted directly to bypass the
-- API's 8-char minimum — intentional, demo only). All app users share it so any
-- of them can sign in with "admin" during the demo.
\set pw '''$2b$12$vvF84n6aTc37fXI1WA2yS.xq1kKdSWFmM3QlmMjOAr6YdUCc3vmjG'''

-- ---------------------------------------------------------------------------
-- 0. Wipe domain data (keep catalog tables seeded by migrations)
-- ---------------------------------------------------------------------------
TRUNCATE
    tournament_payouts, tournament_results, tournament_entries,
    table_seat_assignments, tournament_registrations, tournament_structures,
    tournament_clocks, club_tables, check_in, attendance_streak,
    player_note_tag, player_note, friendship, user_privacy_settings,
    prediction_entry, prediction_point_ledger, player_achievements,
    user_cosmetic, season_pass, quest_completion, season, pro_entitlement,
    club_managers, club_player, tournaments, payout_templates, users
    RESTART IDENTITY CASCADE;
-- Remove clubs with a row-level DELETE (NOT truncate-cascade, which would wipe the
-- global cosmetic_item catalog via its club_id FK). Children are already empty.
DELETE FROM clubs;

-- ---------------------------------------------------------------------------
-- 1. Payout templates (empty after migration — seed tiers covering 2..100)
-- ---------------------------------------------------------------------------
INSERT INTO payout_templates (id, name, min_players, max_players, payout_structure) VALUES
  ('f0000000-0000-0000-0000-000000000001', 'Winner-takes-all (2-8)', 2, 8,
     '[{"position":1,"percentage":100}]'::jsonb),
  ('f0000000-0000-0000-0000-000000000002', 'Top 3 (9-27)', 9, 27,
     '[{"position":1,"percentage":50},{"position":2,"percentage":30},{"position":3,"percentage":20}]'::jsonb),
  ('f0000000-0000-0000-0000-000000000003', 'Top 5 (28-100)', 28, 100,
     '[{"position":1,"percentage":40},{"position":2,"percentage":25},{"position":3,"percentage":15},{"position":4,"percentage":12},{"position":5,"percentage":8}]'::jsonb);

-- ---------------------------------------------------------------------------
-- 2. Club
-- ---------------------------------------------------------------------------
INSERT INTO clubs (id, name, city, country) VALUES
  ('11111111-1111-1111-1111-111111111111', 'VIP Poker Club', 'Harzé', 'BE');

-- ---------------------------------------------------------------------------
-- 3. App users (Bruno/admin + 9 player accounts) + their roster entries
-- ---------------------------------------------------------------------------
INSERT INTO users (id, email, username, first_name, last_name, role, locale, password_hash) VALUES
  ('a0000000-0000-0000-0000-000000000000', 'moyse94@gmail.com', 'bruno',   'Bruno',    'Moyse',     'admin',  'fr', :pw),
  ('a0000000-0000-0000-0000-000000000001', 'antoine.carlier@example.com',  'antoine', 'Antoine',  'Carlier',   'player', 'fr', :pw),
  ('a0000000-0000-0000-0000-000000000002', 'camille.hubert@example.com',   'camille', 'Camille',  'Hubert',    'player', 'fr', :pw),
  ('a0000000-0000-0000-0000-000000000003', 'david.lejeune@example.com',    'david',   'David',    'Lejeune',   'player', 'fr', :pw),
  ('a0000000-0000-0000-0000-000000000004', 'elise.wauters@example.com',    'elise',   'Élise',    'Wauters',   'player', 'nl', :pw),
  ('a0000000-0000-0000-0000-000000000005', 'fabian.goossens@example.com',  'fabian',  'Fabian',   'Goossens',  'player', 'nl', :pw),
  ('a0000000-0000-0000-0000-000000000006', 'gauthier.pirard@example.com',  'gauthier','Gauthier', 'Pirard',    'player', 'fr', :pw),
  ('a0000000-0000-0000-0000-000000000007', 'hugo.marechal@example.com',    'hugo',    'Hugo',     'Maréchal',  'player', 'fr', :pw),
  ('a0000000-0000-0000-0000-000000000008', 'ines.verhoeven@example.com',   'ines',    'Inès',     'Verhoeven', 'player', 'nl', :pw),
  ('a0000000-0000-0000-0000-000000000009', 'julien.demaret@example.com',   'julien',  'Julien',   'Demaret',   'player', 'fr', :pw);

-- Roster entries for the app users (app_user_id links the account to the roster).
-- The roster id mirrors the user id with a 'b' prefix instead of 'a'.
INSERT INTO club_player (id, club_id, display_name, app_user_id)
SELECT ('b' || substr(u.id::text, 2))::uuid,
       '11111111-1111-1111-1111-111111111111',
       u.first_name || ' ' || u.last_name,
       u.id
FROM users u;

-- Bruno is a club manager.
INSERT INTO club_managers (club_id, user_id) VALUES
  ('11111111-1111-1111-1111-111111111111', 'a0000000-0000-0000-0000-000000000000');

-- ---------------------------------------------------------------------------
-- 4. Account-less roster players (31) — real club members without an app account
-- ---------------------------------------------------------------------------
INSERT INTO club_player (id, club_id, display_name, app_user_id)
SELECT ('b0000000-0000-0000-0000-0000000001' || lpad(n::text, 2, '0'))::uuid,
       '11111111-1111-1111-1111-111111111111',
       (ARRAY['Marc','Sophie','Thomas','Julie','Nicolas','Laura','Pierre','Emma','Olivier','Chloé',
              'Maxime','Sarah','Benoît','Léa','Vincent','Manon','Damien','Marie','Sébastien','Audrey',
              'Cédric','Justine','Quentin','Charlotte','Romain','Aurélie','Florian','Céline','Loïc','Élodie','Grégory'])[n]
         || ' ' ||
       (ARRAY['Dubois','Lambert','Martin','Lefèvre','Leroy','Moreau','Simon','Laurent','Michel','Garcia',
              'Dupont','Renard','Lemaire','Fontaine','Henry','Rousseau','Blanc','Girard','Bonnet','Dumont',
              'Robert','Mercier','Boyer','Noël','Petit','Roux','Body','Faure','Gauthier','Marchal','Collin'])[n],
       NULL
FROM generate_series(1, 31) AS n;

-- ---------------------------------------------------------------------------
-- 5. Club tables (6 tables of 9 seats)
-- ---------------------------------------------------------------------------
INSERT INTO club_tables (id, club_id, table_number, max_seats)
SELECT ('e0000000-0000-0000-0000-00000000000' || n)::uuid,
       '11111111-1111-1111-1111-111111111111', n, 9
FROM generate_series(1, 6) AS n;

-- ---------------------------------------------------------------------------
-- 6. Tournaments + registrations + entries + results (procedural)
-- ---------------------------------------------------------------------------
DO $seed$
DECLARE
    v_club CONSTANT uuid := '11111111-1111-1111-1111-111111111111';
    -- Finished tournaments: (id, name, date, field_size)
    fin_ids   uuid[] := ARRAY[
        'd0000000-0000-0000-0000-000000000001',
        'd0000000-0000-0000-0000-000000000002',
        'd0000000-0000-0000-0000-000000000003',
        'd0000000-0000-0000-0000-000000000004',
        'd0000000-0000-0000-0000-000000000005']::uuid[];
    fin_dates date[] := ARRAY['2026-01-03','2026-02-15','2026-03-28','2026-04-25','2026-05-10']::date[];
    fin_field int[]  := ARRAY[31, 42, 28, 26, 30];
    pct       numeric[] := ARRAY[40, 25, 15, 12, 8];  -- top-5 payout percentages
    i int;
    v_tid uuid;
    v_field int;
    v_pool int;
    v_rp uuid;
    v_pos int;
    v_prize int;
    rec record;
BEGIN
    -- ===== Finished tournaments =====
    FOR i IN 1 .. array_length(fin_ids, 1) LOOP
        v_tid   := fin_ids[i];
        v_field := fin_field[i];
        v_pool  := v_field * 2500;  -- buy-in €25; voucher excluded from pool

        INSERT INTO tournaments (id, club_id, name, description, start_time, end_time,
                                 buy_in_cents, rake_cents, voucher_value_cents, seat_cap, live_status)
        VALUES (v_tid, v_club,
                'VIP Weekly #' || i,
                'Tournoi hebdomadaire du VIP Poker Club',
                fin_dates[i] + time '19:30', fin_dates[i] + time '23:30',
                2500, 0, 1000, 60, 'not_started');

        -- Field: pick v_field roster players, ordered deterministically per tournament.
        FOR rec IN
            SELECT rp.id AS rp_id,
                   row_number() OVER (ORDER BY md5(rp.id::text || v_tid::text)) AS rn
            FROM club_player rp
            WHERE rp.club_id = v_club
            ORDER BY rn
            LIMIT v_field
        LOOP
            v_rp  := rec.rp_id;
            v_pos := rec.rn;  -- finishing position 1..N (deterministic but arbitrary)

            INSERT INTO tournament_registrations (tournament_id, club_player_id, status, registration_time)
            VALUES (v_tid, v_rp, 'busted', fin_dates[i] + time '19:00');

            -- Buy-in + mandatory voucher entries.
            INSERT INTO tournament_entries (tournament_id, club_player_id, entry_type, amount_cents, chips_received)
            VALUES (v_tid, v_rp, 'initial', 2500, 20000);
            INSERT INTO tournament_entries (tournament_id, club_player_id, entry_type, amount_cents, chips_received)
            VALUES (v_tid, v_rp, 'voucher', 1000, NULL);

            -- Prize for the top 5; 0 otherwise.
            IF v_pos <= 5 THEN
                v_prize := floor(v_pool * pct[v_pos] / 100.0);
            ELSE
                v_prize := 0;
            END IF;

            INSERT INTO tournament_results (tournament_id, club_player_id, final_position, prize_cents)
            VALUES (v_tid, v_rp, v_pos, v_prize);
        END LOOP;

        -- Finish it — fires the points trigger to compute leaderboard points.
        UPDATE tournaments SET live_status = 'finished' WHERE id = v_tid;
    END LOOP;
END
$seed$;

-- ---------------------------------------------------------------------------
-- 7. Upcoming tournaments (from the flyers) + registrations + entries
-- ---------------------------------------------------------------------------
DO $up$
DECLARE
    v_club CONSTANT uuid := '11111111-1111-1111-1111-111111111111';
    v_t1 CONSTANT uuid := 'd0000000-0000-0000-0000-000000000011'; -- Tournoi Sans Ante
    v_t2 CONSTANT uuid := 'd0000000-0000-0000-0000-000000000012'; -- 7BELLO ON TOUR
    v_rp uuid;
    rec record;
    n int;
BEGIN
    INSERT INTO tournaments (id, club_id, name, description, start_time,
                             buy_in_cents, rake_cents, voucher_value_cents,
                             early_bird_bonus_chips, level_two_bonus_chips,
                             rebuy_max, addon_chips, addon_price_cents,
                             seat_cap, late_registration_level, live_status)
    VALUES
      (v_t1, v_club, 'Tournoi Sans Ante',
       'Structure sans ante. 20K de tapis de départ. Re-cave x2, add-on 30K.',
       timestamptz '2026-06-12 19:30+02',
       2500, 0, 1000, 5000, 5000, 2, 30000, 2000, 50, 5, 'registration_open'),
      (v_t2, v_club, '7BELLO ON TOUR, powered by BOSS',
       '50K de tapis de départ. 20 min/niveau. Re-cave x1, add-on 50K.',
       timestamptz '2026-06-20 19:00+02',
       4000, 0, 1000, 5000, 5000, 1, 50000, 1000, 60, 6, 'registration_open');

    -- Register ~28 players into Sans Ante (mix of app users + account-less).
    n := 0;
    FOR rec IN
        SELECT rp.id AS rp_id, rp.app_user_id,
               row_number() OVER (ORDER BY md5(rp.id::text || v_t1::text)) AS rn
        FROM club_player rp WHERE rp.club_id = v_club
        ORDER BY rn LIMIT 28
    LOOP
        v_rp := rec.rp_id;
        n := n + 1;
        -- First ~16 are pre-registered & checked-in (early-bird granted); rest just registered.
        IF n <= 16 THEN
            INSERT INTO tournament_registrations (tournament_id, club_player_id, status, early_bird_bonus_awarded)
            VALUES (v_t1, v_rp, 'checked_in', true);
            INSERT INTO tournament_entries (tournament_id, club_player_id, entry_type, amount_cents, chips_received)
            VALUES (v_t1, v_rp, 'initial', 2500, 25000);  -- 20K + 5K early-bird
            INSERT INTO tournament_entries (tournament_id, club_player_id, entry_type, amount_cents)
            VALUES (v_t1, v_rp, 'voucher', 1000);
        ELSE
            INSERT INTO tournament_registrations (tournament_id, club_player_id, status)
            VALUES (v_t1, v_rp, 'registered');
        END IF;
    END LOOP;

    -- Register ~22 players into 7BELLO.
    FOR rec IN
        SELECT rp.id AS rp_id,
               row_number() OVER (ORDER BY md5(rp.id::text || v_t2::text)) AS rn
        FROM club_player rp WHERE rp.club_id = v_club
        ORDER BY rn LIMIT 22
    LOOP
        INSERT INTO tournament_registrations (tournament_id, club_player_id, status)
        VALUES (v_t2, rec.rp_id, 'registered');
    END LOOP;

    -- A friend-registration example: Bruno registered Antoine (roster ...001) into 7BELLO.
    UPDATE tournament_registrations
       SET notes = 'Registered by a friend'
     WHERE tournament_id = v_t2
       AND club_player_id = 'b0000000-0000-0000-0000-000000000001';
END
$up$;

-- ---------------------------------------------------------------------------
-- 8. Live tournament (in progress) — seated players, clock running
-- ---------------------------------------------------------------------------
DO $live$
DECLARE
    v_club CONSTANT uuid := '11111111-1111-1111-1111-111111111111';
    v_tid  CONSTANT uuid := 'd0000000-0000-0000-0000-000000000020';
    v_rp uuid;
    rec record;
    seat int;
    tbl int;
    idx int := 0;
    seated_rps uuid[] := '{}';
BEGIN
    INSERT INTO tournaments (id, club_id, name, description, start_time,
                             buy_in_cents, rake_cents, voucher_value_cents,
                             early_bird_bonus_chips, level_two_bonus_chips,
                             seat_cap, late_registration_level, live_status)
    VALUES (v_tid, v_club, 'VIP Live Tonight',
            'Tournoi en cours, démo du flux live.',
            now() - interval '90 minutes',
            2500, 0, 1000, 5000, 5000, 60, 5, 'in_progress');

    -- Link tables 1 & 2 to the tournament (players are seated there below).
    INSERT INTO tournament_table_assignments (tournament_id, club_table_id, is_active)
    SELECT v_tid, ('e0000000-0000-0000-0000-00000000000' || n)::uuid, true
    FROM generate_series(1, 2) AS n;

    -- 18 seated players across tables 1-2 (9 each).
    FOR rec IN
        SELECT rp.id AS rp_id,
               row_number() OVER (ORDER BY md5(rp.id::text || v_tid::text)) AS rn
        FROM club_player rp WHERE rp.club_id = v_club
        ORDER BY rn LIMIT 18
    LOOP
        v_rp := rec.rp_id;
        idx := idx + 1;
        tbl  := CASE WHEN idx <= 9 THEN 1 ELSE 2 END;
        seat := CASE WHEN idx <= 9 THEN idx ELSE idx - 9 END;

        INSERT INTO tournament_registrations (tournament_id, club_player_id, status)
        VALUES (v_tid, v_rp, 'seated');
        INSERT INTO tournament_entries (tournament_id, club_player_id, entry_type, amount_cents, chips_received)
        VALUES (v_tid, v_rp, 'initial', 2500, 25000);
        INSERT INTO tournament_entries (tournament_id, club_player_id, entry_type, amount_cents)
        VALUES (v_tid, v_rp, 'voucher', 1000);
        INSERT INTO table_seat_assignments (tournament_id, club_player_id, club_table_id, seat_number, stack_size)
        VALUES (v_tid, v_rp,
                ('e0000000-0000-0000-0000-00000000000' || tbl)::uuid, seat, 25000);

        seated_rps := seated_rps || v_rp;
    END LOOP;

    -- Grant the level-2 bonus to the first 10 seated players (bonus entry + flag).
    INSERT INTO tournament_entries (tournament_id, club_player_id, entry_type, amount_cents, chips_received, notes)
    SELECT v_tid, rp, 'bonus', 0, 5000, 'Level-2 early-bird bonus'
    FROM unnest(seated_rps[1:10]) AS rp;
    UPDATE tournament_registrations
       SET level_two_bonus_awarded = true
     WHERE tournament_id = v_tid AND club_player_id = ANY(seated_rps[1:10]);

    -- Start the clock (level 4, running).
    UPDATE tournament_clocks
       SET clock_status = 'running', current_level = 4,
           level_started_at = now() - interval '6 minutes',
           level_end_time = now() + interval '14 minutes'
     WHERE tournament_id = v_tid;
END
$live$;

-- ---------------------------------------------------------------------------
-- 9. Cross-feature data (so nothing is empty for the demo)
-- ---------------------------------------------------------------------------
DO $extra$
DECLARE
    v_club CONSTANT uuid := '11111111-1111-1111-1111-111111111111';
    v_bruno CONSTANT uuid := 'a0000000-0000-0000-0000-000000000000';
    v_note uuid;
BEGIN
    -- Player notes by Bruno on a few players (+ tags + showdown style).
    v_note := gen_random_uuid();
    INSERT INTO player_note (id, author_app_user_id, subject_registered_player_id, body, style)
    VALUES (v_note, v_bruno, 'b0000000-0000-0000-0000-000000000101',
            'Très agressif en position. Bluffe les boards secs.', 'LAG');
    INSERT INTO player_note_tag (note_id, kind, tag) VALUES
      (v_note, 'tag', 'aggressive'),
      (v_note, 'tell', 'Regarde ses jetons quand il bluffe');
    INSERT INTO showdown_observation (note_id, tournament_id, description) VALUES
      (v_note, 'd0000000-0000-0000-0000-000000000005', '3-bet bluff A5s OTB vs UTG, montré au showdown');

    v_note := gen_random_uuid();
    INSERT INTO player_note (id, author_app_user_id, subject_registered_player_id, body, style)
    VALUES (v_note, v_bruno, 'b0000000-0000-0000-0000-000000000003', 'Joueur solide, peu de bluffs.', 'TP');
    INSERT INTO player_note_tag (note_id, kind, tag) VALUES (v_note, 'tag', 'nit');

    -- Friendships: Bruno (roster b..000 / user a..000) with app players.
    -- Accepted with Antoine (perm both ways), Camille (Bruno can register Camille), David (pending).
    INSERT INTO friendship (requester_id, addressee_id, status,
                            requester_allows_addressee_reg, addressee_allows_requester_reg) VALUES
      (v_bruno, 'a0000000-0000-0000-0000-000000000001', 'accepted', true,  true),
      (v_bruno, 'a0000000-0000-0000-0000-000000000002', 'accepted', false, true),
      ('a0000000-0000-0000-0000-000000000004', v_bruno, 'accepted', true, false),
      (v_bruno, 'a0000000-0000-0000-0000-000000000003', 'pending',  false, false);

    -- Privacy / scouting opt-in for app users.
    INSERT INTO user_privacy_settings (app_user_id, share_named_pl, in_scouting_pool)
    SELECT id, true, true FROM users WHERE role = 'player';

    -- Prediction-Points: Bruno has an open bet, a won one, a lost one + ledger balance.
    INSERT INTO prediction_entry (app_user_id, tournament_id, predicted_winner_user_id, stake_points, status, payout_points, resolved_at) VALUES
      (v_bruno, 'd0000000-0000-0000-0000-000000000011', 'a0000000-0000-0000-0000-000000000001', 100, 'open',  0, NULL),
      (v_bruno, 'd0000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000002', 150, 'won', 450, now() - interval '20 days'),
      (v_bruno, 'd0000000-0000-0000-0000-000000000002', 'a0000000-0000-0000-0000-000000000003', 100, 'lost',  0, now() - interval '10 days');
    INSERT INTO prediction_point_ledger (app_user_id, delta, reason, ref_id) VALUES
      (v_bruno, 1000, 'seed', NULL),
      (v_bruno, -150, 'prediction_stake', NULL),
      (v_bruno,  450, 'prediction_payout', NULL),
      (v_bruno, -100, 'prediction_stake', NULL);

    -- Achievements: unlock a few for Bruno and Antoine.
    INSERT INTO player_achievements (user_id, achievement_id, unlocked_at, progress)
    SELECT v_bruno, a.id, now() - interval '30 days', COALESCE(a.threshold_value, 1)
    FROM achievements a WHERE a.code IN ('first_registration', 'first_cash', 'tournaments_5');
    INSERT INTO player_achievements (user_id, achievement_id, unlocked_at, progress)
    SELECT 'a0000000-0000-0000-0000-000000000001', a.id, now() - interval '15 days', COALESCE(a.threshold_value, 1)
    FROM achievements a WHERE a.code IN ('first_registration', 'first_win');

    -- Season + premium pass for Bruno + a completed weekly quest.
    INSERT INTO season (id, club_id, name, starts_at, ends_at) VALUES
      ('aa000000-0000-0000-0000-000000000001', v_club, 'Saison 2026',
       timestamptz '2026-01-01', timestamptz '2026-12-31');
    INSERT INTO season_pass (season_id, app_user_id, is_premium) VALUES
      ('aa000000-0000-0000-0000-000000000001', v_bruno, true);
    INSERT INTO quest_completion (app_user_id, quest_code, week_start, xp_awarded) VALUES
      (v_bruno, 'play_two_tournaments', date '2026-06-01', 100);

    -- Cosmetics: Bruno owns + equips a card back (purchased) and a badge (gift).
    INSERT INTO user_cosmetic (app_user_id, cosmetic_item_id, source, equipped)
    SELECT v_bruno, ci.id, 'purchase', true FROM cosmetic_item ci WHERE ci.code = 'card_back_classic_gold';
    INSERT INTO user_cosmetic (app_user_id, cosmetic_item_id, source, equipped)
    SELECT v_bruno, ci.id, 'club_gift', false FROM cosmetic_item ci WHERE ci.code = 'badge_high_roller';

    -- Pro entitlement (active) for Bruno — unlocks Pro-only field notes.
    INSERT INTO pro_entitlement (app_user_id, source, granted_by_club_id, status, starts_at, expires_at) VALUES
      (v_bruno, 'club_gift', v_club, 'active', now() - interval '30 days', now() + interval '335 days');

    -- Check-ins across finished tournaments for app users → streaks + mutual flames.
    INSERT INTO check_in (app_user_id, tournament_id, club_id, checked_in_at)
    SELECT u.id, t.id, v_club, t.start_time
    FROM users u
    CROSS JOIN tournaments t
    WHERE u.id IN ('a0000000-0000-0000-0000-000000000000',
                   'a0000000-0000-0000-0000-000000000001',
                   'a0000000-0000-0000-0000-000000000002')
      AND t.live_status = 'finished';

    INSERT INTO attendance_streak (app_user_id, current_streak, longest_streak, last_check_in_at) VALUES
      (v_bruno, 5, 5, timestamptz '2026-05-10 19:30+02'),
      ('a0000000-0000-0000-0000-000000000001', 3, 4, timestamptz '2026-05-10 19:30+02'),
      ('a0000000-0000-0000-0000-000000000002', 2, 2, timestamptz '2026-05-10 19:30+02');
END
$extra$;

COMMIT;

-- ---------------------------------------------------------------------------
-- Quick sanity summary
-- ---------------------------------------------------------------------------
SELECT 'clubs' AS t, count(*) FROM clubs
UNION ALL SELECT 'users', count(*) FROM users
UNION ALL SELECT 'club_player', count(*) FROM club_player
UNION ALL SELECT 'tournaments', count(*) FROM tournaments
UNION ALL SELECT 'registrations', count(*) FROM tournament_registrations
UNION ALL SELECT 'entries', count(*) FROM tournament_entries
UNION ALL SELECT 'results', count(*) FROM tournament_results
UNION ALL SELECT 'seat_assignments', count(*) FROM table_seat_assignments
UNION ALL SELECT 'payouts', count(*) FROM tournament_payouts
UNION ALL SELECT 'friendships', count(*) FROM friendship
UNION ALL SELECT 'notes', count(*) FROM player_note
UNION ALL SELECT 'predictions', count(*) FROM prediction_entry;
