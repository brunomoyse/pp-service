-- Remove the existing auto-create structure trigger
DROP TRIGGER IF EXISTS auto_create_structure_trigger ON tournament_clocks;
DROP FUNCTION IF EXISTS auto_create_tournament_structure();

-- Create new trigger function to auto-create tournament clock when tournament is created
CREATE OR REPLACE FUNCTION auto_create_tournament_clock()
RETURNS TRIGGER AS $$
BEGIN
    -- Check if tournament doesn't already have a clock
    IF NOT EXISTS (
        SELECT 1 FROM tournament_clocks 
        WHERE tournament_id = NEW.id
    ) THEN
        -- Create basic tournament clock in stopped state at level 1
        INSERT INTO tournament_clocks (tournament_id, clock_status, current_level, auto_advance)
        VALUES (NEW.id, 'stopped', 1, true);
        
        RAISE INFO 'Auto-created tournament clock for tournament %', NEW.id;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger on tournaments table
CREATE TRIGGER auto_create_clock_trigger
    AFTER INSERT ON tournaments
    FOR EACH ROW
    EXECUTE FUNCTION auto_create_tournament_clock();