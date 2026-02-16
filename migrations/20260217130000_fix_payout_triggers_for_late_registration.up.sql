-- Fix: payout row was only created on transition to IN_PROGRESS,
-- but with the new flow (REGISTRATION_OPEN → LATE_REGISTRATION → IN_PROGRESS),
-- the tournament is already running during LATE_REGISTRATION and entries are being
-- added. The payout row must exist by then so the entry trigger can update it.

-- 1. Update calculate_tournament_payouts to also trigger on entry to LATE_REGISTRATION
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
    -- Proceed when entering LATE_REGISTRATION or IN_PROGRESS
    IF NEW.live_status IN ('late_registration', 'in_progress')
       AND OLD.live_status != NEW.live_status THEN

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

-- 2. Update recalculate_prize_pool_from_entries to CREATE the payout row if missing
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

    -- Calculate totals from entries
    SELECT COALESCE(SUM(amount_cents), 0), COUNT(DISTINCT user_id)
    INTO v_total_amount, v_player_count
    FROM tournament_entries WHERE tournament_id = v_tournament_id;

    -- Find appropriate template based on unique player count
    SELECT * INTO v_template FROM payout_templates
    WHERE min_players <= v_player_count
    AND (max_players IS NULL OR max_players >= v_player_count)
    ORDER BY min_players DESC LIMIT 1;

    -- Calculate payout positions if template found
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

    -- Upsert: update if exists, create if not
    INSERT INTO tournament_payouts (
        tournament_id,
        template_id,
        player_count,
        total_prize_pool,
        payout_positions
    ) VALUES (
        v_tournament_id,
        v_template.id,
        v_player_count,
        v_total_amount,
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
