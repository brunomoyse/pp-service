-- Update existing tournament results to calculate points using the authoritative formula
-- Formula: points = min(60, round(3 * (sqrt(field_size) / sqrt(rank)) * (log10(buy_in_eur) + 1) + 2))

UPDATE tournament_results 
SET points = LEAST(60, 
    ROUND(
        3.0 * (
            SQRT(tournament_info.field_size::float) / SQRT(tournament_results.final_position::float)
        ) * (
            LOG(tournament_info.buy_in_eur) + 1.0
        ) + 2.0
    )::integer
)
FROM (
    SELECT 
        t.id as tournament_id,
        t.buy_in_cents::float / 100.0 as buy_in_eur,
        COUNT(tr_count.user_id)::float as field_size
    FROM tournaments t
    LEFT JOIN tournament_registrations tr_count ON t.id = tr_count.tournament_id
    GROUP BY t.id, t.buy_in_cents
) as tournament_info
WHERE tournament_results.tournament_id = tournament_info.tournament_id
    AND tournament_results.points = 0  -- Only update results that haven't been calculated
    AND tournament_info.field_size > 0  -- Ensure we have registrations
    AND tournament_info.buy_in_eur > 0  -- Ensure positive buy-in
    AND tournament_results.final_position > 0;  -- Ensure valid position

-- Display updated results for verification
SELECT 
    tr.tournament_id,
    tr.user_id,
    tr.final_position,
    tr.prize_cents,
    tr.points,
    t.buy_in_cents,
    COUNT(reg.user_id) as field_size
FROM tournament_results tr
JOIN tournaments t ON tr.tournament_id = t.id
LEFT JOIN tournament_registrations reg ON t.id = reg.tournament_id
WHERE tr.points > 0
GROUP BY tr.id, tr.tournament_id, tr.user_id, tr.final_position, tr.prize_cents, tr.points, t.buy_in_cents
ORDER BY tr.tournament_id, tr.final_position;