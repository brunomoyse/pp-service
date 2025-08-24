-- Create a table to track tournament payouts (calculated from templates)
CREATE TABLE IF NOT EXISTS tournament_payouts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    template_id UUID REFERENCES payout_templates(id),
    player_count INTEGER NOT NULL,
    total_prize_pool INTEGER NOT NULL, -- in cents
    payout_positions JSONB NOT NULL, -- Array of {position: 1, amount_cents: 5000, percentage: 50.0}
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tournament_id)
);

-- Create index for quick lookups
CREATE INDEX idx_tournament_payouts_tournament_id ON tournament_payouts(tournament_id);

-- Function to calculate and create tournament payouts
CREATE OR REPLACE FUNCTION calculate_tournament_payouts()
RETURNS TRIGGER AS $$
DECLARE
    v_player_count INTEGER;
    v_total_prize_pool INTEGER;
    v_template RECORD;
    v_payout_structure JSONB;
    v_payout_positions JSONB;
    v_position RECORD;
    v_positions_array JSONB[];
    v_payout_amount INTEGER;
BEGIN
    -- Only proceed if status changed from LATE_REGISTRATION to IN_PROGRESS
    IF (OLD.live_status = 'late_registration' OR OLD.live_status = 'not_started') 
       AND NEW.live_status = 'in_progress' THEN
        
        -- Check if payouts already exist for this tournament
        IF EXISTS (SELECT 1 FROM tournament_payouts WHERE tournament_id = NEW.id) THEN
            RETURN NEW;
        END IF;
        
        -- Count registered players
        SELECT COUNT(*) INTO v_player_count
        FROM tournament_registrations
        WHERE tournament_id = NEW.id
        AND status = 'pending';
        
        -- Skip if no players
        IF v_player_count = 0 THEN
            RETURN NEW;
        END IF;
        
        -- Calculate total prize pool (buy-in * number of players)
        v_total_prize_pool := NEW.buy_in_cents * v_player_count;
        
        -- Find appropriate payout template based on player count
        SELECT * INTO v_template
        FROM payout_templates
        WHERE min_players <= v_player_count 
        AND (max_players IS NULL OR max_players >= v_player_count)
        ORDER BY min_players DESC
        LIMIT 1;
        
        -- If no template found, log warning and return
        IF v_template.id IS NULL THEN
            RAISE WARNING 'No payout template found for % players in tournament %', v_player_count, NEW.id;
            RETURN NEW;
        END IF;
        
        -- Calculate payout for each position
        v_positions_array := ARRAY[]::JSONB[];
        
        FOR v_position IN 
            SELECT * FROM jsonb_array_elements(v_template.payout_structure)
        LOOP
            -- Extract position and percentage
            v_payout_amount := FLOOR((v_position.value->>'percentage')::NUMERIC * v_total_prize_pool / 100);
            
            v_positions_array := array_append(
                v_positions_array, 
                jsonb_build_object(
                    'position', (v_position.value->>'position')::INTEGER,
                    'amount_cents', v_payout_amount,
                    'percentage', (v_position.value->>'percentage')::NUMERIC
                )
            );
        END LOOP;
        
        v_payout_positions := to_jsonb(v_positions_array);
        
        -- Insert the calculated payouts
        INSERT INTO tournament_payouts (
            tournament_id,
            template_id,
            player_count,
            total_prize_pool,
            payout_positions
        ) VALUES (
            NEW.id,
            v_template.id,
            v_player_count,
            v_total_prize_pool,
            v_payout_positions
        );
        
        RAISE NOTICE 'Created payouts for tournament % with % players using template %', 
            NEW.id, v_player_count, v_template.name;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger on tournaments table
CREATE TRIGGER trg_calculate_tournament_payouts
    AFTER UPDATE OF live_status ON tournaments
    FOR EACH ROW
    EXECUTE FUNCTION calculate_tournament_payouts();

-- Add trigger for updated_at on tournament_payouts
CREATE TRIGGER trg_tournament_payouts_updated_at
    BEFORE UPDATE ON tournament_payouts
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- Calculate payouts for existing tournaments that are already in progress
-- (This handles tournaments that transitioned before this trigger was created)
WITH tournament_counts AS (
    SELECT 
        t.id AS tournament_id,
        t.buy_in_cents,
        COUNT(tr.id) AS player_count,
        t.buy_in_cents * COUNT(tr.id) AS total_prize_pool
    FROM tournaments t
    INNER JOIN tournament_registrations tr ON tr.tournament_id = t.id AND tr.status = 'pending'
    WHERE t.live_status = 'in_progress'
    AND NOT EXISTS (SELECT 1 FROM tournament_payouts tp WHERE tp.tournament_id = t.id)
    GROUP BY t.id, t.buy_in_cents
),
matched_templates AS (
    SELECT 
        tc.tournament_id,
        tc.buy_in_cents,
        tc.player_count,
        tc.total_prize_pool,
        pt.id AS template_id,
        pt.payout_structure
    FROM tournament_counts tc
    CROSS JOIN LATERAL (
        SELECT * FROM payout_templates pt
        WHERE pt.min_players <= tc.player_count 
        AND (pt.max_players IS NULL OR pt.max_players >= tc.player_count)
        ORDER BY pt.min_players DESC
        LIMIT 1
    ) pt
)
INSERT INTO tournament_payouts (tournament_id, template_id, player_count, total_prize_pool, payout_positions)
SELECT 
    tournament_id,
    template_id,
    player_count,
    total_prize_pool,
    (
        SELECT to_jsonb(array_agg(
            jsonb_build_object(
                'position', (pos->>'position')::INTEGER,
                'amount_cents', FLOOR((pos->>'percentage')::NUMERIC * total_prize_pool / 100),
                'percentage', (pos->>'percentage')::NUMERIC
            ) ORDER BY (pos->>'position')::INTEGER
        ))
        FROM jsonb_array_elements(payout_structure) pos
    )
FROM matched_templates
ON CONFLICT DO NOTHING;