-- Create trigger function to auto-create tournament clock
CREATE OR REPLACE FUNCTION auto_create_tournament_clock()
RETURNS TRIGGER AS $$
BEGIN
    -- Only create clock when status changes TO late_registration
    -- (indicating tournament is starting)
    IF NEW.live_status = 'late_registration' AND 
       (OLD.live_status IS NULL OR OLD.live_status != 'late_registration') THEN
        
        -- Check if clock doesn't already exist
        IF NOT EXISTS (
            SELECT 1 FROM tournament_clocks 
            WHERE tournament_id = NEW.id
        ) THEN
            -- Create the tournament clock
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

-- Create the trigger on tournaments table
CREATE TRIGGER auto_create_clock_trigger
    AFTER UPDATE OF live_status ON tournaments
    FOR EACH ROW
    EXECUTE FUNCTION auto_create_tournament_clock();

-- Also create trigger for INSERT (in case tournament is created with late_registration status)
CREATE TRIGGER auto_create_clock_insert_trigger
    AFTER INSERT ON tournaments
    FOR EACH ROW
    WHEN (NEW.live_status = 'late_registration')
    EXECUTE FUNCTION auto_create_tournament_clock();