-- Restore the per-tournament field size (from 20250816140000).
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
    SELECT t.buy_in_cents
    INTO tournament_record
    FROM tournaments t
    WHERE t.id = tournament_id_param;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Tournament not found: %', tournament_id_param;
    END IF;

    SELECT COUNT(*)
    INTO field_size_count
    FROM tournament_registrations tr
    WHERE tr.tournament_id = tournament_id_param;

    IF field_size_count = 0 THEN
        RAISE WARNING 'No registrations found for tournament: %', tournament_id_param;
        RETURN 0;
    END IF;

    buy_in_eur := tournament_record.buy_in_cents::DECIMAL / 100.0;

    IF buy_in_eur <= 0 THEN
        RAISE WARNING 'Invalid buy-in amount for tournament: %', tournament_id_param;
        RETURN 0;
    END IF;

    FOR result_record IN
        SELECT id, final_position
        FROM tournament_results
        WHERE tournament_id = tournament_id_param
          AND final_position > 0
    LOOP
        calculated_points := LEAST(60,
            ROUND(
                3.0 * (
                    SQRT(field_size_count::DECIMAL) / SQRT(result_record.final_position::DECIMAL)
                ) * (
                    LOG(buy_in_eur) + 1.0
                ) + 2.0
            )::INTEGER
        );

        calculated_points := GREATEST(0, calculated_points);

        UPDATE tournament_results
        SET points = calculated_points, updated_at = NOW()
        WHERE id = result_record.id;

        total_updated := total_updated + 1;
    END LOOP;

    RETURN total_updated;
END;
$$ LANGUAGE plpgsql;
