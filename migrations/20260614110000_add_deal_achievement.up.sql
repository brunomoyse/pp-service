-- "Made a deal at the final table" achievement. A deal/chop is recorded as a
-- player_deals row whose affected_positions lists the dealt finishing places; a
-- player earns this if their tournament_results.final_position is among them.
-- Evaluated in gql/domains/achievements/service.rs. icon = cut-outline (a chop).
INSERT INTO achievements (code, name_key, description_key, category, icon, tier, threshold_value)
VALUES
    ('final_table_deal', 'achievements.items.final_table_deal.name', 'achievements.items.final_table_deal.description', 'results', 'cut-outline', 'silver', NULL)
ON CONFLICT (code) DO NOTHING;
