-- Series-aware points field size.
--
-- A multi-day series is scored ONCE, on the final day, but ranked against the
-- WHOLE field — not just the Day 2 survivors. Override the field size in
-- calculate_tournament_points: for a final-day tournament, use the distinct
-- entrant count across every flight of the series; otherwise keep the
-- per-tournament registration count (single-day behaviour, unchanged).

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
    SELECT t.buy_in_cents, t.series_id, t.is_final_day
    INTO tournament_record
    FROM tournaments t
    WHERE t.id = tournament_id_param;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Tournament not found: %', tournament_id_param;
    END IF;

    -- Field size: for a series final day, the distinct entrants across all
    -- flights; otherwise this tournament's registrations.
    IF tournament_record.is_final_day AND tournament_record.series_id IS NOT NULL THEN
        SELECT COUNT(DISTINCT te.club_player_id)
        INTO field_size_count
        FROM tournament_entries te
        JOIN tournaments t ON t.id = te.tournament_id
        WHERE t.series_id = tournament_record.series_id
          AND te.entry_type NOT IN ('voucher', 'bonus');
    ELSE
        SELECT COUNT(*)
        INTO field_size_count
        FROM tournament_registrations tr
        WHERE tr.tournament_id = tournament_id_param;
    END IF;

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

    RAISE INFO 'Updated % tournament results with calculated points for tournament %', total_updated, tournament_id_param;
    RETURN total_updated;
END;
$$ LANGUAGE plpgsql;
