-- Drop trigger first
DROP TRIGGER IF EXISTS trg_calculate_tournament_payouts ON tournaments;

-- Drop the function
DROP FUNCTION IF EXISTS calculate_tournament_payouts();

-- Drop the updated_at trigger
DROP TRIGGER IF EXISTS trg_tournament_payouts_updated_at ON tournament_payouts;

-- Drop indexes
DROP INDEX IF EXISTS idx_tournament_payouts_tournament_id;

-- Drop the table
DROP TABLE IF EXISTS tournament_payouts;