-- Create trigger function to auto-create basic tournament structure
CREATE OR REPLACE FUNCTION auto_create_tournament_structure()
RETURNS TRIGGER AS $$
BEGIN
    -- Check if tournament doesn't already have structure levels
    IF NOT EXISTS (
        SELECT 1 FROM tournament_structures 
        WHERE tournament_id = NEW.tournament_id
    ) THEN
        -- Create basic tournament structure (common poker tournament levels)
        INSERT INTO tournament_structures (tournament_id, level_number, small_blind, big_blind, ante, duration_minutes) VALUES
            (NEW.tournament_id, 1, 25, 50, 0, 20),
            (NEW.tournament_id, 2, 50, 100, 0, 20), 
            (NEW.tournament_id, 3, 75, 150, 25, 20),
            (NEW.tournament_id, 4, 100, 200, 25, 20),
            (NEW.tournament_id, 5, 150, 300, 50, 20),
            (NEW.tournament_id, 6, 200, 400, 50, 20),
            (NEW.tournament_id, 7, 300, 600, 75, 20),
            (NEW.tournament_id, 8, 400, 800, 100, 20),
            (NEW.tournament_id, 9, 500, 1000, 100, 20),
            (NEW.tournament_id, 10, 600, 1200, 200, 20),
            -- Add a break after level 10
            (NEW.tournament_id, 11, 0, 0, 0, 15), -- 15 minute break
            (NEW.tournament_id, 12, 800, 1600, 200, 20),
            (NEW.tournament_id, 13, 1000, 2000, 300, 20),
            (NEW.tournament_id, 14, 1500, 3000, 500, 20),
            (NEW.tournament_id, 15, 2000, 4000, 500, 20);
            
        -- Update the break level to be marked as break
        UPDATE tournament_structures 
        SET is_break = true, break_duration_minutes = 15
        WHERE tournament_id = NEW.tournament_id AND level_number = 11;
        
        RAISE INFO 'Auto-created basic tournament structure for tournament %', NEW.tournament_id;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger on tournament_clocks table
CREATE TRIGGER auto_create_structure_trigger
    AFTER INSERT ON tournament_clocks
    FOR EACH ROW
    EXECUTE FUNCTION auto_create_tournament_structure();