-- Inter-club achievements: reward players for playing across multiple clubs,
-- to encourage circulating between venues. Distinct-club count is evaluated in
-- gql/domains/achievements/service.rs. Category reuses 'milestones'.
INSERT INTO achievements (code, name_key, description_key, category, icon, tier, threshold_value)
VALUES
    ('clubs_2', 'achievements.items.clubs_2.name', 'achievements.items.clubs_2.description', 'milestones', 'globe-outline', 'bronze', 2),
    ('clubs_3', 'achievements.items.clubs_3.name', 'achievements.items.clubs_3.description', 'milestones', 'compass-outline', 'silver', 3),
    ('clubs_5', 'achievements.items.clubs_5.name', 'achievements.items.clubs_5.description', 'milestones', 'earth-outline', 'gold', 5)
ON CONFLICT (code) DO NOTHING;
