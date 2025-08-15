-- Drop indexes
DROP INDEX IF EXISTS tournament_results_created_at_idx;
DROP INDEX IF EXISTS tournament_results_user_id_idx;
DROP INDEX IF EXISTS tournament_results_tournament_id_idx;

-- Drop table
DROP TABLE IF EXISTS tournament_results;