-- Remove live state columns from tournaments table
ALTER TABLE tournaments 
DROP COLUMN IF EXISTS current_level,
DROP COLUMN IF EXISTS players_remaining;

-- Create tournament_state table for live tournament data
CREATE TABLE tournament_state (
    id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    tournament_id UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    current_level INTEGER,
    players_remaining INTEGER,
    break_until TIMESTAMPTZ,
    current_small_blind INTEGER,
    current_big_blind INTEGER,
    current_ante INTEGER,
    level_started_at TIMESTAMPTZ,
    level_duration_minutes INTEGER DEFAULT 20,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    
    CONSTRAINT unique_tournament_state UNIQUE (tournament_id)
);

-- Add trigger for updated_at
CREATE TRIGGER trg_tournament_state_updated_at
    BEFORE UPDATE ON tournament_state
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- Create index for efficient lookups
CREATE INDEX idx_tournament_state_tournament_id ON tournament_state(tournament_id);