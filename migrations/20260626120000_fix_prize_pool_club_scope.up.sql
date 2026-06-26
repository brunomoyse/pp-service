-- Fix a regression introduced by 20260621130000_club_scope_templates: that
-- migration rewrote recalculate_prize_pool_from_entries from an outdated body,
-- silently dropping four behaviours that later migrations had added:
--   1. excluding voucher/bonus entries from the prize pool,
--   2. counting distinct players by club_player_id (user_id is NULL for
--      account-less roster players),
--   3. subtracting the bounty slice from the pool,
--   4. the multi-day series final-day aggregation across all flights.
-- This restores the canonical body (from 20260611170000_series_prize_pool) and,
-- because templates are now club-owned, also scopes apply_tournament_payout's
-- template selection to the tournament's club.

-- 1. apply_tournament_payout: select a payout template owned by the tournament's
-- club (templates became club-scoped in 20260621130000).
CREATE OR REPLACE FUNCTION apply_tournament_payout(
    p_tournament_id UUID,
    p_total_amount  INTEGER,
    p_player_count  INTEGER
) RETURNS VOID AS $$
DECLARE
    v_club_id UUID;
    v_template RECORD;
    v_payout_positions JSONB;
BEGIN
    SELECT club_id INTO v_club_id FROM tournaments WHERE id = p_tournament_id;

    SELECT * INTO v_template FROM payout_templates
    WHERE club_id = v_club_id
    AND min_players <= p_player_count
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

-- 2. recalculate_prize_pool_from_entries: canonical body restored.
CREATE OR REPLACE FUNCTION recalculate_prize_pool_from_entries()
RETURNS TRIGGER AS $$
DECLARE
    v_tournament_id UUID;
    v_series_id UUID;
    v_final_day_id UUID;
    v_total_amount INTEGER;
    v_player_count INTEGER;
    v_bounty_slice INTEGER;
BEGIN
    v_tournament_id := COALESCE(NEW.tournament_id, OLD.tournament_id);

    SELECT series_id INTO v_series_id FROM tournaments WHERE id = v_tournament_id;

    -- (1) The changed tournament's own per-night payout (single-day path,
    -- also the per-flight cash desk). Vouchers and bonuses are excluded from the
    -- prize pool; players are counted by club_player_id (account-less safe).
    SELECT
        COALESCE(SUM(amount_cents) FILTER (WHERE entry_type NOT IN ('voucher', 'bonus')), 0),
        COUNT(DISTINCT club_player_id) FILTER (WHERE entry_type NOT IN ('voucher', 'bonus'))
    INTO v_total_amount, v_player_count
    FROM tournament_entries WHERE tournament_id = v_tournament_id;

    SELECT COALESCE(bounty_amount_cents, 0) * COUNT(*) FILTER (
        WHERE te.entry_type IN ('initial', 'rebuy', 're_entry'))
    INTO v_bounty_slice
    FROM tournaments t
    LEFT JOIN tournament_entries te ON te.tournament_id = t.id
    WHERE t.id = v_tournament_id
    GROUP BY t.bounty_amount_cents;

    v_total_amount := GREATEST(v_total_amount - COALESCE(v_bounty_slice, 0), 0);
    PERFORM apply_tournament_payout(v_tournament_id, v_total_amount, v_player_count);

    -- (2) If this tournament belongs to a series, refresh the final day's
    -- aggregate across all flights.
    IF v_series_id IS NOT NULL THEN
        SELECT id INTO v_final_day_id FROM tournaments
        WHERE series_id = v_series_id AND is_final_day = TRUE LIMIT 1;

        IF v_final_day_id IS NOT NULL THEN
            SELECT
                COALESCE(SUM(te.amount_cents) FILTER (WHERE te.entry_type NOT IN ('voucher', 'bonus')), 0),
                COUNT(DISTINCT te.club_player_id) FILTER (WHERE te.entry_type NOT IN ('voucher', 'bonus'))
            INTO v_total_amount, v_player_count
            FROM tournament_entries te
            JOIN tournaments t ON t.id = te.tournament_id
            WHERE t.series_id = v_series_id;

            SELECT COALESCE(SUM(sub.slice), 0) INTO v_bounty_slice FROM (
                SELECT t.bounty_amount_cents * COUNT(*) FILTER (
                    WHERE te.entry_type IN ('initial', 'rebuy', 're_entry')) AS slice
                FROM tournaments t
                JOIN tournament_entries te ON te.tournament_id = t.id
                WHERE t.series_id = v_series_id AND t.bounty_amount_cents > 0
                GROUP BY t.id, t.bounty_amount_cents
            ) sub;

            v_total_amount := GREATEST(v_total_amount - COALESCE(v_bounty_slice, 0), 0);
            PERFORM apply_tournament_payout(v_final_day_id, v_total_amount, v_player_count);
        END IF;
    END IF;

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;
