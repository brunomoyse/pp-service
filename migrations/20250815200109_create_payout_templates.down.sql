-- Drop indexes
DROP INDEX IF EXISTS player_deals_created_by_idx;
DROP INDEX IF EXISTS player_deals_tournament_id_idx;
DROP INDEX IF EXISTS payout_templates_players_idx;
DROP INDEX IF EXISTS payout_templates_name_idx;

-- Drop tables
DROP TABLE IF EXISTS player_deals;
DROP TABLE IF EXISTS payout_templates;