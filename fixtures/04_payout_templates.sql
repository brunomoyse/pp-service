-- Payout templates

INSERT INTO payout_templates (name, description, min_players, max_players, payout_structure)
VALUES
    ('Heads Up', 'Winner takes all for 2 players', 2, 2,
     '[{"position": 1, "percentage": 100.0}]'::jsonb),

    ('3-10 Players', 'Top 2 paid', 3, 10,
     '[{"position": 1, "percentage": 70.0}, {"position": 2, "percentage": 30.0}]'::jsonb),

    ('11-20 Players', 'Top 3 paid', 11, 20,
     '[{"position": 1, "percentage": 50.0}, {"position": 2, "percentage": 30.0}, {"position": 3, "percentage": 20.0}]'::jsonb),

    ('21-30 Players', 'Top 5 paid', 21, 30,
     '[{"position": 1, "percentage": 37.0}, {"position": 2, "percentage": 25.0}, {"position": 3, "percentage": 15.0}, {"position": 4, "percentage": 12.0}, {"position": 5, "percentage": 11.0}]'::jsonb),

    ('31-40 Players', 'Top 6 paid', 31, 40,
     '[{"position": 1, "percentage": 35.0}, {"position": 2, "percentage": 22.0}, {"position": 3, "percentage": 15.0}, {"position": 4, "percentage": 11.0}, {"position": 5, "percentage": 9.0}, {"position": 6, "percentage": 8.0}]'::jsonb),

    ('41-50 Players', 'Top 8 paid', 41, 50,
     '[{"position": 1, "percentage": 32.0}, {"position": 2, "percentage": 18.0}, {"position": 3, "percentage": 12.5}, {"position": 4, "percentage": 10.5}, {"position": 5, "percentage": 8.3}, {"position": 6, "percentage": 7.3}, {"position": 7, "percentage": 6.2}, {"position": 8, "percentage": 5.2}]'::jsonb),

    ('51-60 Players', 'Top 9 paid', 51, 60,
     '[{"position": 1, "percentage": 30.0}, {"position": 2, "percentage": 17.5}, {"position": 3, "percentage": 12.2}, {"position": 4, "percentage": 10.2}, {"position": 5, "percentage": 8.1}, {"position": 6, "percentage": 7.1}, {"position": 7, "percentage": 6.1}, {"position": 8, "percentage": 5.1}, {"position": 9, "percentage": 3.7}]'::jsonb),

    ('61-75 Players', 'Top 10 paid', 61, 75,
     '[{"position": 1, "percentage": 29.0}, {"position": 2, "percentage": 17.0}, {"position": 3, "percentage": 12.0}, {"position": 4, "percentage": 10.0}, {"position": 5, "percentage": 8.0}, {"position": 6, "percentage": 6.9}, {"position": 7, "percentage": 5.9}, {"position": 8, "percentage": 4.9}, {"position": 9, "percentage": 3.5}, {"position": 10, "percentage": 2.8}]'::jsonb),

    ('76+ Players', 'Top 10 paid', 76, NULL,
     '[{"position": 1, "percentage": 28.0}, {"position": 2, "percentage": 16.0}, {"position": 3, "percentage": 11.5}, {"position": 4, "percentage": 9.5}, {"position": 5, "percentage": 7.5}, {"position": 6, "percentage": 6.5}, {"position": 7, "percentage": 5.5}, {"position": 8, "percentage": 4.5}, {"position": 9, "percentage": 3.0}, {"position": 10, "percentage": 2.5}, {"position": 11, "percentage": 2.0}, {"position": 12, "percentage": 1.5}, {"position": 13, "percentage": 1.0}, {"position": 14, "percentage": 0.7}, {"position": 15, "percentage": 0.3}]'::jsonb)
ON CONFLICT DO NOTHING;
