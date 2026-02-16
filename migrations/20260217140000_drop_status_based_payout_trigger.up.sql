-- Prize pool is now calculated entirely from entries (the source of truth for money in).
-- The status-based trigger is no longer needed since the entry trigger upserts.

DROP TRIGGER IF EXISTS trg_calculate_tournament_payouts ON tournaments;
DROP FUNCTION IF EXISTS calculate_tournament_payouts();
