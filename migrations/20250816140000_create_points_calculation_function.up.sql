-- Create function to calculate tournament points using the authoritative formula
-- Formula: points = min(60, round(3 * (sqrt(field_size) / sqrt(rank)) * (log10(buy_in_eur) + 1) + 2))

CREATE OR REPLACE FUNCTION calculate_tournament_points(tournament_id_param UUID)
RETURNS INTEGER AS $$
DECLARE
    tournament_record RECORD;
    field_size_count INTEGER;
    buy_in_eur DECIMAL;
    result_record RECORD;
    calculated_points INTEGER;
    total_updated INTEGER := 0;
BEGIN
    -- Get tournament information
    SELECT t.buy_in_cents 
    INTO tournament_record
    FROM tournaments t 
    WHERE t.id = tournament_id_param;
    
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Tournament not found: %', tournament_id_param;
    END IF;
    
    -- Calculate field size from registrations
    SELECT COUNT(*)
    INTO field_size_count
    FROM tournament_registrations tr
    WHERE tr.tournament_id = tournament_id_param;
    
    IF field_size_count = 0 THEN
        RAISE WARNING 'No registrations found for tournament: %', tournament_id_param;
        RETURN 0;
    END IF;
    
    -- Convert buy-in to euros
    buy_in_eur := tournament_record.buy_in_cents::DECIMAL / 100.0;
    
    IF buy_in_eur <= 0 THEN
        RAISE WARNING 'Invalid buy-in amount for tournament: %', tournament_id_param;
        RETURN 0;
    END IF;
    
    -- Calculate points for each result
    FOR result_record IN 
        SELECT id, final_position 
        FROM tournament_results 
        WHERE tournament_id = tournament_id_param
          AND final_position > 0
    LOOP
        -- Apply the authoritative formula
        -- points = min(60, round(3 * (sqrt(field_size) / sqrt(rank)) * (log10(buy_in_eur) + 1) + 2))
        calculated_points := LEAST(60, 
            ROUND(
                3.0 * (
                    SQRT(field_size_count::DECIMAL) / SQRT(result_record.final_position::DECIMAL)
                ) * (
                    LOG(buy_in_eur) + 1.0
                ) + 2.0
            )::INTEGER
        );
        
        -- Ensure non-negative points
        calculated_points := GREATEST(0, calculated_points);
        
        -- Update the result with calculated points
        UPDATE tournament_results 
        SET points = calculated_points, updated_at = NOW()
        WHERE id = result_record.id;
        
        total_updated := total_updated + 1;
    END LOOP;
    
    RAISE INFO 'Updated % tournament results with calculated points for tournament %', total_updated, tournament_id_param;
    RETURN total_updated;
END;
$$ LANGUAGE plpgsql;

-- Create trigger function that fires when tournament status changes to finished
CREATE OR REPLACE FUNCTION trigger_calculate_points()
RETURNS TRIGGER AS $$
BEGIN
    -- Only calculate points when tournament moves to finished status
    IF NEW.live_status = 'finished' AND (OLD.live_status IS NULL OR OLD.live_status != 'finished') THEN
        PERFORM calculate_tournament_points(NEW.id);
        RAISE INFO 'Points calculated for tournament % due to status change to finished', NEW.id;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create the trigger on tournaments table
DROP TRIGGER IF EXISTS tournament_status_points_trigger ON tournaments;
CREATE TRIGGER tournament_status_points_trigger
    AFTER UPDATE OF live_status ON tournaments
    FOR EACH ROW
    EXECUTE FUNCTION trigger_calculate_points();

-- Also create a manual function to recalculate points for any tournament
CREATE OR REPLACE FUNCTION recalculate_all_tournament_points(tournament_id_param UUID DEFAULT NULL)
RETURNS INTEGER AS $$
DECLARE
    tournament_id_to_process UUID;
    total_tournaments INTEGER := 0;
BEGIN
    IF tournament_id_param IS NOT NULL THEN
        -- Recalculate for specific tournament
        PERFORM calculate_tournament_points(tournament_id_param);
        RETURN 1;
    ELSE
        -- Recalculate for all tournaments
        FOR tournament_id_to_process IN 
            SELECT DISTINCT t.id 
            FROM tournaments t 
            INNER JOIN tournament_results tr ON t.id = tr.tournament_id
        LOOP
            PERFORM calculate_tournament_points(tournament_id_to_process);
            total_tournaments := total_tournaments + 1;
        END LOOP;
        
        RAISE INFO 'Recalculated points for % tournaments', total_tournaments;
        RETURN total_tournaments;
    END IF;
END;
$$ LANGUAGE plpgsql;