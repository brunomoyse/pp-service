-- Complete the registered_player → club_player rename.
--
-- The rename migration (20260610130000) renamed the FK column on the four
-- operational tables (registrations, results, entries, seat assignments) but
-- missed two tables created in the days before it:
--   * drink_wallet.registered_player_id   (20260608140000)
--   * player_note.subject_registered_player_id (20260606121000)
-- The repo layer (drink_wallets.rs, player_notes.rs) already selects/inserts the
-- club_player_id names, so the columns must follow the rename too.

ALTER TABLE drink_wallet RENAME COLUMN registered_player_id TO club_player_id;
ALTER INDEX drink_wallet_registered_player_id_idx RENAME TO drink_wallet_club_player_id_idx;

ALTER TABLE player_note RENAME COLUMN subject_registered_player_id TO subject_club_player_id;
