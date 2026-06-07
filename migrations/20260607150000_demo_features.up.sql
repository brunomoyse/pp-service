-- Stage 2 demo features: mandatory drink voucher, dual early-bird bonus,
-- flyer config (rebuy cap / add-on), and friend-registration permission.

-- 2.1 Mandatory drink voucher. A paper voucher IRL: its value is collected from
-- the player but EXCLUDED from the prize pool (the club keeps it). Stored in cents.
ALTER TABLE tournaments
    ADD COLUMN voucher_value_cents INTEGER NOT NULL DEFAULT 0;

-- 2.2 Second early-bird bonus, granted to players still seated at the end of L2.
-- (early_bird_bonus_chips already exists for the pre-registration bonus.)
ALTER TABLE tournaments
    ADD COLUMN level_two_bonus_chips INTEGER DEFAULT NULL;

-- Track per-registration which bonuses were already awarded (idempotency).
ALTER TABLE tournament_registrations
    ADD COLUMN early_bird_bonus_awarded BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE tournament_registrations
    ADD COLUMN level_two_bonus_awarded BOOLEAN NOT NULL DEFAULT false;

-- 2.4 Flyer config: rebuy cap and add-on chips/price so the apps can display
-- the tournament's rebuy/add-on terms. All optional.
ALTER TABLE tournaments
    ADD COLUMN rebuy_max INTEGER DEFAULT NULL;
ALTER TABLE tournaments
    ADD COLUMN addon_chips INTEGER DEFAULT NULL;
ALTER TABLE tournaments
    ADD COLUMN addon_price_cents INTEGER DEFAULT NULL;

-- 2.3 Friend-registration permission. Per-direction: each friend independently
-- decides whether the other may register them into a tournament.
ALTER TABLE friendship
    ADD COLUMN requester_allows_addressee_reg BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE friendship
    ADD COLUMN addressee_allows_requester_reg BOOLEAN NOT NULL DEFAULT false;

-- Allow the two new entry types: 'voucher' (drink voucher, excluded from pool)
-- and 'bonus' (chip-only grants like the level-2 early-bird bonus).
ALTER TABLE tournament_entries
    DROP CONSTRAINT IF EXISTS tournament_entries_entry_type_check;
ALTER TABLE tournament_entries
    ADD CONSTRAINT tournament_entries_entry_type_check
    CHECK (entry_type IN ('initial', 'rebuy', 're_entry', 'addon', 'voucher', 'bonus'));

-- Recompute the prize pool excluding voucher and bonus entries from both the
-- money total and the player count (add-ons still count toward the pool).
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
        COUNT(DISTINCT registered_player_id) FILTER (WHERE entry_type NOT IN ('voucher', 'bonus'))
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
