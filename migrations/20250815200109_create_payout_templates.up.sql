-- Create payout_templates table for tournament payout structures
CREATE TABLE payout_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL, -- e.g., "Standard 9-handed", "Turbo Structure"
    description TEXT,
    min_players INTEGER NOT NULL DEFAULT 2,
    max_players INTEGER, -- NULL means no maximum
    payout_structure JSONB NOT NULL, -- Array of {position: 1, percentage: 50.0} objects
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create player_deals table for final table deals that override payout structure
CREATE TABLE player_deals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    deal_type VARCHAR(50) NOT NULL DEFAULT 'even_split', -- 'even_split', 'icm', 'custom'
    affected_positions INTEGER[] NOT NULL, -- e.g., [1,2,3] for top 3 players
    custom_payouts JSONB, -- For custom deals: {user_id: amount_cents}
    total_amount_cents INTEGER NOT NULL, -- Total amount being dealt
    notes TEXT,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Ensure one deal per tournament
    UNIQUE(tournament_id)
);

-- Add indexes
CREATE INDEX payout_templates_name_idx ON payout_templates (name);
CREATE INDEX payout_templates_players_idx ON payout_templates (min_players, max_players);
CREATE INDEX player_deals_tournament_id_idx ON player_deals (tournament_id);
CREATE INDEX player_deals_created_by_idx ON player_deals (created_by);

-- Add triggers for updated_at
CREATE TRIGGER trg_payout_templates_updated_at
    BEFORE UPDATE ON payout_templates
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

CREATE TRIGGER trg_player_deals_updated_at
    BEFORE UPDATE ON player_deals
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();