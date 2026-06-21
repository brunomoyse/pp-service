-- Club-scoped templates: every blind structure / payout template now belongs to
-- a club. A full default set is seeded per club (and on every newly created club
-- via trigger), so each manager starts with an editable set and never sees
-- another club's templates.

-- 1. Add club_id (nullable first; backfilled below, then made NOT NULL).
ALTER TABLE blind_structure_templates
    ADD COLUMN club_id UUID REFERENCES clubs(id) ON DELETE CASCADE;
ALTER TABLE payout_templates
    ADD COLUMN club_id UUID REFERENCES clubs(id) ON DELETE CASCADE;

-- 2. Seed function: inserts the default template set for one club. Single source
-- of truth, reused by the backfill below and the per-club trigger.
CREATE OR REPLACE FUNCTION seed_club_default_templates(p_club_id UUID)
RETURNS VOID AS $$
BEGIN
    INSERT INTO blind_structure_templates (club_id, name, description, levels) VALUES
    (
        p_club_id,
        'Quick Tournament (1h)',
        'Fast-paced tournament with 10-minute levels. Perfect for casual games.',
        '[
            {"levelNumber": 1, "smallBlind": 25, "bigBlind": 50, "ante": 0, "durationMinutes": 10, "isBreak": false},
            {"levelNumber": 2, "smallBlind": 50, "bigBlind": 100, "ante": 0, "durationMinutes": 10, "isBreak": false},
            {"levelNumber": 3, "smallBlind": 100, "bigBlind": 200, "ante": 0, "durationMinutes": 10, "isBreak": false},
            {"levelNumber": 4, "smallBlind": 200, "bigBlind": 400, "ante": 0, "durationMinutes": 10, "isBreak": false},
            {"levelNumber": 5, "smallBlind": 400, "bigBlind": 800, "ante": 0, "durationMinutes": 10, "isBreak": false},
            {"levelNumber": 6, "smallBlind": 600, "bigBlind": 1200, "ante": 0, "durationMinutes": 10, "isBreak": false}
        ]'::jsonb
    ),
    (
        p_club_id,
        'Standard Tournament (2h)',
        'Classic structure with 15-minute levels and one break. Balanced for most home games.',
        '[
            {"levelNumber": 1, "smallBlind": 25, "bigBlind": 50, "ante": 0, "durationMinutes": 15, "isBreak": false},
            {"levelNumber": 2, "smallBlind": 50, "bigBlind": 100, "ante": 0, "durationMinutes": 15, "isBreak": false},
            {"levelNumber": 3, "smallBlind": 75, "bigBlind": 150, "ante": 0, "durationMinutes": 15, "isBreak": false},
            {"levelNumber": 4, "smallBlind": 100, "bigBlind": 200, "ante": 0, "durationMinutes": 15, "isBreak": false},
            {"levelNumber": 5, "smallBlind": 0, "bigBlind": 0, "ante": 0, "durationMinutes": 15, "isBreak": true, "breakDurationMinutes": 15},
            {"levelNumber": 6, "smallBlind": 150, "bigBlind": 300, "ante": 25, "durationMinutes": 15, "isBreak": false},
            {"levelNumber": 7, "smallBlind": 200, "bigBlind": 400, "ante": 50, "durationMinutes": 15, "isBreak": false},
            {"levelNumber": 8, "smallBlind": 300, "bigBlind": 600, "ante": 75, "durationMinutes": 15, "isBreak": false},
            {"levelNumber": 9, "smallBlind": 400, "bigBlind": 800, "ante": 100, "durationMinutes": 15, "isBreak": false}
        ]'::jsonb
    ),
    (
        p_club_id,
        'Deep Stack (3h+)',
        'Longer levels for skilled play with gradual blind progression. Ideal for serious tournaments.',
        '[
            {"levelNumber": 1, "smallBlind": 25, "bigBlind": 50, "ante": 0, "durationMinutes": 20, "isBreak": false},
            {"levelNumber": 2, "smallBlind": 50, "bigBlind": 100, "ante": 0, "durationMinutes": 20, "isBreak": false},
            {"levelNumber": 3, "smallBlind": 75, "bigBlind": 150, "ante": 0, "durationMinutes": 20, "isBreak": false},
            {"levelNumber": 4, "smallBlind": 100, "bigBlind": 200, "ante": 0, "durationMinutes": 20, "isBreak": false},
            {"levelNumber": 5, "smallBlind": 0, "bigBlind": 0, "ante": 0, "durationMinutes": 15, "isBreak": true, "breakDurationMinutes": 15},
            {"levelNumber": 6, "smallBlind": 150, "bigBlind": 300, "ante": 25, "durationMinutes": 20, "isBreak": false},
            {"levelNumber": 7, "smallBlind": 200, "bigBlind": 400, "ante": 50, "durationMinutes": 20, "isBreak": false},
            {"levelNumber": 8, "smallBlind": 300, "bigBlind": 600, "ante": 75, "durationMinutes": 20, "isBreak": false},
            {"levelNumber": 9, "smallBlind": 0, "bigBlind": 0, "ante": 0, "durationMinutes": 15, "isBreak": true, "breakDurationMinutes": 15},
            {"levelNumber": 10, "smallBlind": 400, "bigBlind": 800, "ante": 100, "durationMinutes": 20, "isBreak": false},
            {"levelNumber": 11, "smallBlind": 600, "bigBlind": 1200, "ante": 150, "durationMinutes": 20, "isBreak": false},
            {"levelNumber": 12, "smallBlind": 800, "bigBlind": 1600, "ante": 200, "durationMinutes": 20, "isBreak": false}
        ]'::jsonb
    );

    INSERT INTO payout_templates (club_id, name, description, min_players, max_players, payout_structure) VALUES
    (
        p_club_id, 'Winner Takes All', 'Single prize for first place. Best for small fields.', 2, 8,
        '[{"position": 1, "percentage": 100}]'::jsonb
    ),
    (
        p_club_id, 'Top 3 (50/30/20)', 'Classic three-place payout.', 9, 18,
        '[{"position": 1, "percentage": 50}, {"position": 2, "percentage": 30}, {"position": 3, "percentage": 20}]'::jsonb
    ),
    (
        p_club_id, 'Top 5 (40/25/15/12/8)', 'Pays the final five.', 19, 30,
        '[{"position": 1, "percentage": 40}, {"position": 2, "percentage": 25}, {"position": 3, "percentage": 15}, {"position": 4, "percentage": 12}, {"position": 5, "percentage": 8}]'::jsonb
    ),
    (
        p_club_id, 'Top 9 (final table)', 'Deep payout for large fields.', 31, NULL,
        '[{"position": 1, "percentage": 30}, {"position": 2, "percentage": 20}, {"position": 3, "percentage": 14}, {"position": 4, "percentage": 10}, {"position": 5, "percentage": 8}, {"position": 6, "percentage": 6}, {"position": 7, "percentage": 5}, {"position": 8, "percentage": 4}, {"position": 9, "percentage": 3}]'::jsonb
    );
END;
$$ LANGUAGE plpgsql;

-- 3. Backfill: drop the legacy global rows and seed every existing club.
DELETE FROM blind_structure_templates WHERE club_id IS NULL;
DELETE FROM payout_templates WHERE club_id IS NULL;
DO $$
DECLARE c RECORD;
BEGIN
    FOR c IN SELECT id FROM clubs LOOP
        PERFORM seed_club_default_templates(c.id);
    END LOOP;
END $$;

-- 4. Enforce NOT NULL now that every template is club-owned.
ALTER TABLE blind_structure_templates ALTER COLUMN club_id SET NOT NULL;
ALTER TABLE payout_templates ALTER COLUMN club_id SET NOT NULL;

CREATE INDEX idx_blind_structure_templates_club ON blind_structure_templates(club_id);
CREATE INDEX idx_payout_templates_club ON payout_templates(club_id);

-- 5. Auto-seed the default templates for every newly created club.
CREATE OR REPLACE FUNCTION trg_seed_club_default_templates()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM seed_club_default_templates(NEW.id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER after_insert_club_seed_templates
    AFTER INSERT ON clubs
    FOR EACH ROW EXECUTE FUNCTION trg_seed_club_default_templates();

-- 6. Scope the payout auto-calculation triggers to the tournament's club, so a
-- tournament only ever picks one of its own club's payout templates.
CREATE OR REPLACE FUNCTION calculate_tournament_payouts()
RETURNS TRIGGER AS $$
DECLARE
    v_player_count INTEGER;
    v_total_prize_pool INTEGER;
    v_template RECORD;
    v_payout_positions JSONB;
    v_position RECORD;
    v_positions_array JSONB[];
    v_payout_amount INTEGER;
BEGIN
    IF NEW.live_status IN ('late_registration', 'in_progress')
       AND OLD.live_status != NEW.live_status THEN

        IF EXISTS (SELECT 1 FROM tournament_payouts WHERE tournament_id = NEW.id) THEN
            RETURN NEW;
        END IF;

        SELECT COUNT(*) INTO v_player_count
        FROM tournament_registrations
        WHERE tournament_id = NEW.id
        AND status IN ('registered', 'checked_in', 'seated', 'busted');

        IF v_player_count = 0 THEN
            RETURN NEW;
        END IF;

        v_total_prize_pool := NEW.buy_in_cents * v_player_count;

        -- Find a payout template owned by this tournament's club.
        SELECT * INTO v_template
        FROM payout_templates
        WHERE club_id = NEW.club_id
        AND min_players <= v_player_count
        AND (max_players IS NULL OR max_players >= v_player_count)
        ORDER BY min_players DESC
        LIMIT 1;

        IF v_template.id IS NULL THEN
            RAISE WARNING 'No payout template found for % players in tournament %', v_player_count, NEW.id;
            RETURN NEW;
        END IF;

        v_positions_array := ARRAY[]::JSONB[];

        FOR v_position IN
            SELECT * FROM jsonb_array_elements(v_template.payout_structure)
        LOOP
            v_payout_amount := FLOOR((v_position.value->>'percentage')::NUMERIC * v_total_prize_pool / 100);

            v_positions_array := array_append(
                v_positions_array,
                jsonb_build_object(
                    'position', (v_position.value->>'position')::INTEGER,
                    'amount_cents', v_payout_amount,
                    'percentage', (v_position.value->>'percentage')::NUMERIC
                )
            );
        END LOOP;

        v_payout_positions := to_jsonb(v_positions_array);

        INSERT INTO tournament_payouts (
            tournament_id, template_id, player_count, total_prize_pool, payout_positions
        ) VALUES (
            NEW.id, v_template.id, v_player_count, v_total_prize_pool, v_payout_positions
        );

        RAISE NOTICE 'Created payouts for tournament % with % players using template %',
            NEW.id, v_player_count, v_template.name;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION recalculate_prize_pool_from_entries()
RETURNS TRIGGER AS $$
DECLARE
    v_tournament_id UUID;
    v_club_id UUID;
    v_total_amount INTEGER;
    v_player_count INTEGER;
    v_template RECORD;
    v_payout_positions JSONB;
BEGIN
    v_tournament_id := COALESCE(NEW.tournament_id, OLD.tournament_id);

    SELECT club_id INTO v_club_id FROM tournaments WHERE id = v_tournament_id;

    SELECT COALESCE(SUM(amount_cents), 0), COUNT(DISTINCT user_id)
    INTO v_total_amount, v_player_count
    FROM tournament_entries WHERE tournament_id = v_tournament_id;

    -- Find a payout template owned by this tournament's club.
    SELECT * INTO v_template FROM payout_templates
    WHERE club_id = v_club_id
    AND min_players <= v_player_count
    AND (max_players IS NULL OR max_players >= v_player_count)
    ORDER BY min_players DESC LIMIT 1;

    IF v_template.id IS NOT NULL THEN
        SELECT to_jsonb(array_agg(
            jsonb_build_object(
                'position', (pos->>'position')::INTEGER,
                'amount_cents', FLOOR((pos->>'percentage')::NUMERIC * v_total_amount / 100),
                'percentage', (pos->>'percentage')::NUMERIC
            ) ORDER BY (pos->>'position')::INTEGER
        )) INTO v_payout_positions
        FROM jsonb_array_elements(v_template.payout_structure) pos;
    ELSE
        v_payout_positions := '[]'::JSONB;
    END IF;

    INSERT INTO tournament_payouts (
        tournament_id, template_id, player_count, total_prize_pool, payout_positions
    ) VALUES (
        v_tournament_id, v_template.id, v_player_count, v_total_amount,
        COALESCE(v_payout_positions, '[]'::JSONB)
    )
    ON CONFLICT (tournament_id) DO UPDATE SET
        total_prize_pool = EXCLUDED.total_prize_pool,
        player_count = EXCLUDED.player_count,
        template_id = EXCLUDED.template_id,
        payout_positions = EXCLUDED.payout_positions,
        updated_at = NOW();

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;
