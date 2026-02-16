-- Fix: the calculate_tournament_payouts trigger was counting registrations
-- with status = 'pending', but that status does not exist.
-- The actual confirmed statuses are: registered, checked_in, seated, busted.
-- Also update the backfill CTE query used in the original migration (no longer needed,
-- but the trigger function itself is the critical fix).

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

        -- Check if payouts already exist for this tournament
        IF EXISTS (SELECT 1 FROM tournament_payouts WHERE tournament_id = NEW.id) THEN
            RETURN NEW;
        END IF;

        -- Count confirmed players (registered, checked_in, seated, busted)
        SELECT COUNT(*) INTO v_player_count
        FROM tournament_registrations
        WHERE tournament_id = NEW.id
        AND status IN ('registered', 'checked_in', 'seated', 'busted');

        -- Skip if no players
        IF v_player_count = 0 THEN
            RETURN NEW;
        END IF;

        -- Calculate total prize pool (buy-in * number of players)
        v_total_prize_pool := NEW.buy_in_cents * v_player_count;

        -- Find appropriate payout template based on player count
        SELECT * INTO v_template
        FROM payout_templates
        WHERE min_players <= v_player_count
        AND (max_players IS NULL OR max_players >= v_player_count)
        ORDER BY min_players DESC
        LIMIT 1;

        -- If no template found, log warning and return
        IF v_template.id IS NULL THEN
            RAISE WARNING 'No payout template found for % players in tournament %', v_player_count, NEW.id;
            RETURN NEW;
        END IF;

        -- Calculate payout for each position
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

        -- Insert the calculated payouts
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

        RAISE NOTICE 'Created payouts for tournament % with % players using template %',
            NEW.id, v_player_count, v_template.name;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
