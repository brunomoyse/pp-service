-- Tournament Entries - tracks all buy-ins including initial, rebuys, re-entries, and add-ons
CREATE TABLE tournament_entries (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id   UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    entry_type      TEXT NOT NULL CHECK (entry_type IN ('initial', 'rebuy', 're_entry', 'addon')),
    amount_cents    INTEGER NOT NULL,
    chips_received  INTEGER,
    recorded_by     UUID REFERENCES users(id),
    notes           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for common queries
CREATE INDEX idx_tournament_entries_tournament_id ON tournament_entries(tournament_id);
CREATE INDEX idx_tournament_entries_user_id ON tournament_entries(user_id);
CREATE INDEX idx_tournament_entries_entry_type ON tournament_entries(entry_type);
CREATE INDEX idx_tournament_entries_created_at ON tournament_entries(created_at);

-- Trigger for updated_at
CREATE TRIGGER trg_tournament_entries_updated_at
    BEFORE UPDATE ON tournament_entries
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- Function to recalculate prize pool when entries change
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

    -- Update tournament_payouts if it exists
    IF EXISTS (SELECT 1 FROM tournament_payouts WHERE tournament_id = v_tournament_id) THEN
        -- Find appropriate template based on unique player count
        SELECT * INTO v_template FROM payout_templates
        WHERE min_players <= v_player_count
        AND (max_players IS NULL OR max_players >= v_player_count)
        ORDER BY min_players DESC LIMIT 1;

        -- Recalculate payout positions if template found
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

-- Trigger to recalculate prize pool on entry changes
CREATE TRIGGER trg_recalculate_prize_pool
    AFTER INSERT OR UPDATE OR DELETE ON tournament_entries
    FOR EACH ROW
    EXECUTE FUNCTION recalculate_prize_pool_from_entries();
