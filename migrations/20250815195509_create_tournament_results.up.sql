-- Create tournament_results table to track player results and prize winnings
CREATE TABLE tournament_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    final_position INTEGER NOT NULL, -- 1st, 2nd, 3rd, etc.
    prize_cents INTEGER NOT NULL DEFAULT 0, -- Prize amount in cents
    notes TEXT, -- Additional notes about the result
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Ensure one result per user per tournament
    UNIQUE(tournament_id, user_id)
);

-- Add indexes for efficient queries
CREATE INDEX tournament_results_tournament_id_idx ON tournament_results (tournament_id);
CREATE INDEX tournament_results_user_id_idx ON tournament_results (user_id);
CREATE INDEX tournament_results_created_at_idx ON tournament_results (created_at);

-- Add trigger for updated_at
CREATE TRIGGER trg_tournament_results_updated_at
    BEFORE UPDATE ON tournament_results
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();