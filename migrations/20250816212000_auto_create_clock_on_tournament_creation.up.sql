-- First, drop the existing trigger that creates clock on status change
DROP TRIGGER IF EXISTS auto_create_clock_insert_trigger ON tournaments;
DROP TRIGGER IF EXISTS auto_create_clock_trigger ON tournaments;

-- Update the trigger function to create clock on ANY tournament creation
CREATE OR REPLACE FUNCTION auto_create_tournament_clock()
RETURNS TRIGGER AS $$
BEGIN
    -- Create tournament clock for every new tournament
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
    
    RAISE INFO 'Auto-created tournament clock for new tournament %', NEW.id;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger on tournament INSERT only
CREATE TRIGGER auto_create_clock_on_creation_trigger
    AFTER INSERT ON tournaments
    FOR EACH ROW
    EXECUTE FUNCTION auto_create_tournament_clock();