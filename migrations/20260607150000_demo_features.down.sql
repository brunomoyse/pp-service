-- Reverse Stage 2 demo features.

-- Restore the prize-pool function to the roster-identity version (no voucher/bonus filter).
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

    SELECT COALESCE(SUM(amount_cents), 0), COUNT(DISTINCT registered_player_id)
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

ALTER TABLE tournament_entries
    DROP CONSTRAINT IF EXISTS tournament_entries_entry_type_check;
ALTER TABLE tournament_entries
    ADD CONSTRAINT tournament_entries_entry_type_check
    CHECK (entry_type IN ('initial', 'rebuy', 're_entry', 'addon'));

ALTER TABLE friendship DROP COLUMN IF EXISTS addressee_allows_requester_reg;
ALTER TABLE friendship DROP COLUMN IF EXISTS requester_allows_addressee_reg;

ALTER TABLE tournaments DROP COLUMN IF EXISTS addon_price_cents;
ALTER TABLE tournaments DROP COLUMN IF EXISTS addon_chips;
ALTER TABLE tournaments DROP COLUMN IF EXISTS rebuy_max;

ALTER TABLE tournament_registrations DROP COLUMN IF EXISTS level_two_bonus_awarded;
ALTER TABLE tournament_registrations DROP COLUMN IF EXISTS early_bird_bonus_awarded;

ALTER TABLE tournaments DROP COLUMN IF EXISTS level_two_bonus_chips;
ALTER TABLE tournaments DROP COLUMN IF EXISTS voucher_value_cents;
