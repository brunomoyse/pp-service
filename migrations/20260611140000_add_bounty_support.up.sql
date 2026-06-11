-- Bounty / progressive knockout (PKO) support.

-- 1. Tournament bounty configuration. bounty_amount_cents is the slice of each
--    buy-in (and rebuy / re-entry) diverted from the prize pool to the bounty pool.
ALTER TABLE tournaments
    ADD COLUMN bounty_type TEXT NOT NULL DEFAULT 'none'
        CHECK (bounty_type IN ('none', 'fixed', 'progressive')),
    ADD COLUMN bounty_amount_cents INTEGER NOT NULL DEFAULT 0;

-- 2. Progressive head value per player. Each bounty-funding entry adds
--    bounty_amount_cents; a knockout takes half and grows the hunter's own head.
ALTER TABLE tournament_registrations
    ADD COLUMN current_bounty_cents INTEGER NOT NULL DEFAULT 0;

-- 3. Per-knockout audit: who collected what off whom.
CREATE TABLE tournament_bounties (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id         UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    hunter_club_player_id UUID NOT NULL REFERENCES club_player(id),
    victim_club_player_id UUID NOT NULL REFERENCES club_player(id),
    amount_cents          INTEGER NOT NULL,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_tournament_bounties_tournament ON tournament_bounties (tournament_id);
CREATE INDEX idx_tournament_bounties_hunter ON tournament_bounties (hunter_club_player_id);

-- 4. Total bounty cash a player walked away with (filled when results are entered).
ALTER TABLE tournament_results
    ADD COLUMN bounty_winnings_cents INTEGER NOT NULL DEFAULT 0;

-- 5. Prize pool recalculation now deducts the bounty slice. Bounty-funding entry
--    types are initial / rebuy / re_entry (add-ons and vouchers never post a head).
CREATE OR REPLACE FUNCTION recalculate_prize_pool_from_entries()
RETURNS TRIGGER AS $$
DECLARE
    v_tournament_id UUID;
    v_total_amount INTEGER;
    v_player_count INTEGER;
    v_bounty_amount INTEGER;
    v_bounty_entry_count INTEGER;
    v_template RECORD;
    v_payout_positions JSONB;
BEGIN
    v_tournament_id := COALESCE(NEW.tournament_id, OLD.tournament_id);

    SELECT
        COALESCE(SUM(amount_cents) FILTER (WHERE entry_type NOT IN ('voucher', 'bonus')), 0),
        COUNT(DISTINCT club_player_id) FILTER (WHERE entry_type NOT IN ('voucher', 'bonus'))
    INTO v_total_amount, v_player_count
    FROM tournament_entries WHERE tournament_id = v_tournament_id;

    -- Remove the bounty slice from the prize pool (one slice per funding entry).
    SELECT COALESCE(bounty_amount_cents, 0) INTO v_bounty_amount
    FROM tournaments WHERE id = v_tournament_id;

    IF v_bounty_amount > 0 THEN
        SELECT COUNT(*) FILTER (WHERE entry_type IN ('initial', 'rebuy', 're_entry'))
        INTO v_bounty_entry_count
        FROM tournament_entries WHERE tournament_id = v_tournament_id;

        v_total_amount := GREATEST(v_total_amount - (v_bounty_amount * v_bounty_entry_count), 0);
    END IF;

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

-- 6. Grow a player's progressive head on each bounty-funding entry.
CREATE OR REPLACE FUNCTION grow_bounty_head_on_entry()
RETURNS TRIGGER AS $$
DECLARE
    v_bounty_type TEXT;
    v_bounty_amount INTEGER;
BEGIN
    SELECT bounty_type, COALESCE(bounty_amount_cents, 0)
    INTO v_bounty_type, v_bounty_amount
    FROM tournaments WHERE id = NEW.tournament_id;

    IF v_bounty_type = 'progressive'
       AND v_bounty_amount > 0
       AND NEW.entry_type IN ('initial', 'rebuy', 're_entry') THEN
        UPDATE tournament_registrations
        SET current_bounty_cents = current_bounty_cents + v_bounty_amount,
            updated_at = NOW()
        WHERE tournament_id = NEW.tournament_id
          AND club_player_id = NEW.club_player_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_grow_bounty_head
    AFTER INSERT ON tournament_entries
    FOR EACH ROW EXECUTE FUNCTION grow_bounty_head_on_entry();
