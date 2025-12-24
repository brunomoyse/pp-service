DROP TRIGGER IF EXISTS trg_recalculate_prize_pool ON tournament_entries;
DROP FUNCTION IF EXISTS recalculate_prize_pool_from_entries();
DROP TRIGGER IF EXISTS trg_tournament_entries_updated_at ON tournament_entries;
DROP TABLE IF EXISTS tournament_entries;
