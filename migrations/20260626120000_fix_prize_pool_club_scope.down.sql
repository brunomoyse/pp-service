-- Revert to the state left by 20260621130000_club_scope_templates.

-- apply_tournament_payout without club scoping (pre-fix).
CREATE OR REPLACE FUNCTION apply_tournament_payout(
    p_tournament_id UUID,
    p_total_amount  INTEGER,
    p_player_count  INTEGER
) RETURNS VOID AS $$
DECLARE
    v_template RECORD;
    v_payout_positions JSONB;
BEGIN
    SELECT * INTO v_template FROM payout_templates
    WHERE min_players <= p_player_count
    AND (max_players IS NULL OR max_players >= p_player_count)
    ORDER BY min_players DESC LIMIT 1;

    IF v_template.id IS NOT NULL THEN
        SELECT to_jsonb(array_agg(
            jsonb_build_object(
                'position', (pos->>'position')::INTEGER,
                'amount_cents', FLOOR((pos->>'percentage')::NUMERIC * p_total_amount / 100),
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
        p_tournament_id, v_template.id, p_player_count, p_total_amount,
        COALESCE(v_payout_positions, '[]'::JSONB)
    )
    ON CONFLICT (tournament_id) DO UPDATE SET
        total_prize_pool = EXCLUDED.total_prize_pool,
        player_count = EXCLUDED.player_count,
        template_id = EXCLUDED.template_id,
        payout_positions = EXCLUDED.payout_positions,
        updated_at = NOW();
END;
$$ LANGUAGE plpgsql;

-- recalculate_prize_pool_from_entries inline body (pre-fix).
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
