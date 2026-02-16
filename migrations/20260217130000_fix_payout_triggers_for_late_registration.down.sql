-- Revert to the previous version of these trigger functions
-- (from migration 20260216120000_fix_payout_trigger_status_filter)

CREATE OR REPLACE FUNCTION calculate_tournament_payouts()
RETURNS TRIGGER AS $$
DECLARE
    v_player_count INTEGER;
    v_total_prize_pool INTEGER;
    v_template RECORD;
    v_payout_structure JSONB;
    v_payout_positions JSONB;
    v_position RECORD;
    v_positions_array JSONB[];
    v_payout_amount INTEGER;
BEGIN
    -- Only proceed if status changed to IN_PROGRESS
    IF (OLD.live_status = 'late_registration' OR OLD.live_status = 'not_started')
       AND NEW.live_status = 'in_progress' THEN

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
            tournament_id,
            template_id,
            player_count,
            total_prize_pool,
            payout_positions
        ) VALUES (
            NEW.id,
            v_template.id,
            v_player_count,
            v_total_prize_pool,
            v_payout_positions
        );
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Revert entry trigger to only update existing rows
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

    IF EXISTS (SELECT 1 FROM tournament_payouts WHERE tournament_id = v_tournament_id) THEN
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

        UPDATE tournament_payouts
        SET total_prize_pool = v_total_amount,
            player_count = v_player_count,
            payout_positions = COALESCE(v_payout_positions, '[]'::JSONB),
            updated_at = NOW()
        WHERE tournament_id = v_tournament_id;
    END IF;

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;
