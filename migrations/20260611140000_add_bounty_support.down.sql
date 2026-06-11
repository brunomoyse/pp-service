DROP TRIGGER IF EXISTS trg_grow_bounty_head ON tournament_entries;
DROP FUNCTION IF EXISTS grow_bounty_head_on_entry();

-- Restore the prize-pool recalc without the bounty deduction.
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

    SELECT
        COALESCE(SUM(amount_cents) FILTER (WHERE entry_type NOT IN ('voucher', 'bonus')), 0),
        COUNT(DISTINCT club_player_id) FILTER (WHERE entry_type NOT IN ('voucher', 'bonus'))
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

ALTER TABLE tournament_results DROP COLUMN IF EXISTS bounty_winnings_cents;
DROP TABLE IF EXISTS tournament_bounties;
ALTER TABLE tournament_registrations DROP COLUMN IF EXISTS current_bounty_cents;
ALTER TABLE tournaments
    DROP COLUMN IF EXISTS bounty_type,
    DROP COLUMN IF EXISTS bounty_amount_cents;
