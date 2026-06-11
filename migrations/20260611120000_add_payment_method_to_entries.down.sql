DROP INDEX IF EXISTS idx_tournament_entries_payment_method;
ALTER TABLE tournament_entries DROP COLUMN IF EXISTS payment_method;
