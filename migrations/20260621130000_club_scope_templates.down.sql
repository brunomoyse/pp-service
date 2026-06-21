-- Revert club-scoped templates back to the global model.

DROP TRIGGER IF EXISTS after_insert_club_seed_templates ON clubs;
DROP FUNCTION IF EXISTS trg_seed_club_default_templates();
DROP FUNCTION IF EXISTS seed_club_default_templates(UUID);

-- Restore the un-scoped payout trigger functions.
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

        SELECT * INTO v_template
        FROM payout_templates
        WHERE min_players <= v_player_count
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
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION recalculate_prize_pool_from_entries()
RETURNS TRIGGER AS $$
DECLARE
    v_tournament_id UUID;
    v_total_amount INTEGER;
    v_player_count INTEGER;
    v_template RECORD;
    v_payout_positions JSONB;
BEGIN
    v_tournament_id := COALESCE(NEW.tournament_id, OLD.tournament_id);

    SELECT COALESCE(SUM(amount_cents), 0), COUNT(DISTINCT user_id)
    INTO v_total_amount, v_player_count
    FROM tournament_entries WHERE tournament_id = v_tournament_id;

    SELECT * INTO v_template FROM payout_templates
    WHERE min_players <= v_player_count
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

DROP INDEX IF EXISTS idx_blind_structure_templates_club;
DROP INDEX IF EXISTS idx_payout_templates_club;

-- Dropping the columns removes all club-owned template rows.
ALTER TABLE blind_structure_templates DROP COLUMN IF EXISTS club_id;
ALTER TABLE payout_templates DROP COLUMN IF EXISTS club_id;

-- Re-seed the original global blind structure templates.
INSERT INTO blind_structure_templates (name, description, levels) VALUES
(
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
);
