-- Clubs and club tables

INSERT INTO clubs (id, name, city, country) VALUES
    ('66666666-6666-6666-6666-666666666666', 'Poker One', 'Charleroi', 'BE'),
    ('cccccccc-cccc-cccc-cccc-cccccccccccc', 'Liège Poker Club', 'Liège', 'BE'),
    ('dddddddd-dddd-dddd-dddd-dddddddddddd', 'Pokah Room Antwerp', 'Antwerp', 'BE');

INSERT INTO club_tables (id, club_id, table_number, max_seats) VALUES
    ('11111111-1111-1111-1111-111111111111', '66666666-6666-6666-6666-666666666666', 1, 9),
    ('11111111-1111-1111-1111-111111111112', '66666666-6666-6666-6666-666666666666', 2, 9),
    ('11111111-1111-1111-1111-111111111113', '66666666-6666-6666-6666-666666666666', 3, 8),
    ('11111111-1111-1111-1111-111111111114', '66666666-6666-6666-6666-666666666666', 4, 6);
