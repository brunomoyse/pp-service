INSERT INTO clubs (id, name, city, country)
VALUES
    (gen_random_uuid(), 'Liège Poker Club', 'Liège', 'BE'),
    (gen_random_uuid(), 'Pokah Room', 'Antwerpen', 'BE');

WITH c AS (
    SELECT id FROM clubs WHERE name = 'Liège Poker Club' LIMIT 1
    )
INSERT INTO tournaments (id, club_id, name, description, start_time, buy_in_cents, seat_cap, location)
SELECT
    gen_random_uuid(), c.id,
    'Friday Night Freezeout',
    'Classic freezeout, friendly structure.',
    NOW() + INTERVAL '5 days',
    2500,   -- €25.00
    60,
    'Main Hall'
FROM c;

INSERT INTO tags (id, slug, label)
VALUES
    (gen_random_uuid(), 'freezeout', 'Freezeout')
    ON CONFLICT (slug) DO NOTHING;

-- Link the created tourney to the tag
WITH t AS (
    SELECT t.id AS tid
    FROM tournaments t
    WHERE LOWER(t.name) = 'friday night freezeout'
    ORDER BY t.created_at DESC
    LIMIT 1
    ),
    g AS (
SELECT id AS tag_id FROM tags WHERE slug = 'freezeout'
    )
INSERT INTO tournament_tags (tournament_id, tag_id)
SELECT t.tid, g.tag_id FROM t, g
    ON CONFLICT DO NOTHING;