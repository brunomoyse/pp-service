-- Tournament blind structure
CREATE TABLE tournament_structures (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    level_number INTEGER NOT NULL,
    small_blind INTEGER NOT NULL,
    big_blind INTEGER NOT NULL,
    ante INTEGER DEFAULT 0,
    duration_minutes INTEGER NOT NULL DEFAULT 20,
    is_break BOOLEAN DEFAULT false,
    break_duration_minutes INTEGER, -- Only for break levels
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    
    CONSTRAINT unique_tournament_level UNIQUE (tournament_id, level_number)
);

-- Tournament clock state
CREATE TABLE tournament_clocks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    clock_status TEXT NOT NULL CHECK (clock_status IN ('stopped', 'running', 'paused')) DEFAULT 'stopped',
    current_level INTEGER NOT NULL DEFAULT 1,
    level_started_at TIMESTAMPTZ,
    level_end_time TIMESTAMPTZ, -- Calculated end time for current level
    pause_started_at TIMESTAMPTZ, -- When current pause started
    total_pause_duration INTERVAL DEFAULT '0 seconds', -- Total pause time for current level
    auto_advance BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    
    CONSTRAINT unique_tournament_clock UNIQUE (tournament_id)
);

-- Clock events for audit trail
CREATE TABLE tournament_clock_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL CHECK (event_type IN ('start', 'pause', 'resume', 'level_advance', 'manual_advance', 'stop', 'reset')),
    level_number INTEGER,
    manager_id UUID REFERENCES users(id),
    event_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'
);

-- Add updated_at trigger for tournament_clocks
CREATE TRIGGER trg_tournament_clocks_updated_at
    BEFORE UPDATE ON tournament_clocks
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- Indexes for performance
CREATE INDEX idx_tournament_structures_tournament_id ON tournament_structures(tournament_id);
CREATE INDEX idx_tournament_structures_level ON tournament_structures(tournament_id, level_number);
CREATE INDEX idx_tournament_clocks_tournament_id ON tournament_clocks(tournament_id);
CREATE INDEX idx_tournament_clocks_status ON tournament_clocks(clock_status);