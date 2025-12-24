-- Revert performance indexes

DROP INDEX IF EXISTS idx_tournament_clocks_auto_advance_poll;
DROP INDEX IF EXISTS idx_tournaments_starting_soon;
DROP INDEX IF EXISTS idx_seat_assignments_occupied;
