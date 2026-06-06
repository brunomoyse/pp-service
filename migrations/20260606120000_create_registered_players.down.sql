DROP TRIGGER IF EXISTS trg_link_registered_player_registrations ON tournament_registrations;
DROP TRIGGER IF EXISTS trg_link_registered_player_results ON tournament_results;
DROP FUNCTION IF EXISTS link_registered_player();

DROP INDEX IF EXISTS tournament_registrations_registered_player_id_idx;
DROP INDEX IF EXISTS tournament_results_registered_player_id_idx;

ALTER TABLE tournament_registrations DROP COLUMN IF EXISTS registered_player_id;
ALTER TABLE tournament_results DROP COLUMN IF EXISTS registered_player_id;

DROP TABLE IF EXISTS registered_player CASCADE;
