-- Three new achievements, evaluated in gql/domains/achievements/service.rs:
--   bridesmaid  — finished runner-up (2nd place) at least once.  icon = medal-outline.
--   bubble_boy  — busted one spot out of the money (last-paid + 1). icon = ellipse-outline.
--   club_pillar — played in 12 different calendar months.         icon = hourglass-outline.
INSERT INTO achievements (code, name_key, description_key, category, icon, tier, threshold_value)
VALUES
    ('bridesmaid', 'achievements.items.bridesmaid.name', 'achievements.items.bridesmaid.description', 'results', 'medal-outline', 'silver', 1),
    ('bubble_boy', 'achievements.items.bubble_boy.name', 'achievements.items.bubble_boy.description', 'results', 'ellipse-outline', 'bronze', NULL),
    ('club_pillar', 'achievements.items.club_pillar.name', 'achievements.items.club_pillar.description', 'milestones', 'hourglass-outline', 'platinum', 12)
ON CONFLICT (code) DO NOTHING;
