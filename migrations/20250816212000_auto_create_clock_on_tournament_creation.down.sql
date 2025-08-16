-- Drop the trigger
DROP TRIGGER IF EXISTS auto_create_clock_on_creation_trigger ON tournaments;

-- Restore the original trigger function (for rollback)
CREATE OR REPLACE FUNCTION auto_create_tournament_clock()
RETURNS TRIGGER AS $$
BEGIN
    -- Only create clock when status changes TO late_registration
    IF NEW.live_status = 'late_registration' AND 
       (OLD.live_status IS NULL OR OLD.live_status != 'late_registration') THEN
        
        IF NOT EXISTS (
            SELECT 1 FROM tournament_clocks 
            WHERE tournament_id = NEW.id
        ) THEN
            INSERT INTO tournament_clocks (
                tournament_id, 
                clock_status, 
                current_level,
                auto_advance
            ) VALUES (
                NEW.id,
                'stopped',
                1,
                true
            );
            
            RAISE INFO 'Auto-created tournament clock for tournament %', NEW.id;
        END IF;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;