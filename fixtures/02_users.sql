-- Users and club managers

INSERT INTO users (id, email, username, first_name, last_name, phone, role, is_active, password_hash) VALUES
    -- Club Managers
    ('ffffffff-ffff-ffff-ffff-ffffffffffff', 'contact@brunomoyse.be', null, 'Bruno', 'Moyse', '+32477123456', 'manager', true, '$2b$12$/5HEs8VNRQjb9olil8qV2.7FpVXROsv4ZRhqhG6qlhGiTG8A/GK86'),
    -- Global Admin
    ('f0f0f0f0-f0f0-f0f0-f0f0-f0f0f0f0f0f0', 'admin@pocketpair.be', 'super_admin', 'Admin', 'Global', '+32477999999', 'admin', true, '$2b$12$/5HEs8VNRQjb9olil8qV2.7FpVXROsv4ZRhqhG6qlhGiTG8A/GK86'),
    -- Belgian Players
    ('30303030-3030-3030-3030-303030303030', 'damien@email.com', 'ace_killer', 'Damien', 'Hupé', '+32477567890', 'player', true, null),
    ('40404040-4040-4040-4040-404040404040', 'rico@email.com', 'poker_king', 'Rico', 'Chevalot', '+32477678901', 'player', true, null),
    ('50505050-5050-5050-5050-505050505050', 'aliosha@email.com', 'bluff_master', 'Aliosha', 'Staes', '+32477789012', 'player', true, null),
    ('60606060-6060-6060-6060-606060606060', 'rami@email.com', 'lucky_rami', 'Rami', 'Awad', '+32477890123', 'player', true, null),
    ('70707070-7070-7070-7070-707070707070', 'jeanmarie@email.com', 'card_shark', 'Jean-Marie', 'Vandeborne', '+32477901234', 'player', true, null),
    ('80808080-8080-8080-8080-808080808080', 'guillaume@email.com', 'river_master', 'Guillaume', 'Gillet', '+32478012345', 'player', true, null),
    ('90909090-9090-9090-9090-909090909090', 'manu@email.com', 'all_in_manu', 'Manu', 'Lecomte', '+32478123456', 'player', true, null),
    ('a0a0a0a0-a0a0-a0a0-a0a0-a0a0a0a0a0a0', 'fabien@email.com', 'tight_tiger', 'Fabien', 'Perrot', '+32478234567', 'player', true, null),
    ('b0b0b0b0-b0b0-b0b0-b0b0-b0b0b0b0b0b0', 'luca@email.com', 'fold_expert', 'Luca', 'Pecoraro', '+32478345678', 'player', true, null),
    ('c0c0c0c0-c0c0-c0c0-c0c0-c0c0c0c0c0c0', 'david@email.com', 'nuts_hunter', 'David', 'Opdebeek', '+32478456789', 'player', true, null),
    ('d0d0d0d0-d0d0-d0d0-d0d0-d0d0d0d0d0d0', 'danny@email.com', 'pocket_rockets', 'Danny', 'Covyn', '+32478567890', 'player', true, null),
    ('e0e0e0e0-e0e0-e0e0-e0e0-e0e0e0e0e0e0', 'sebastien@email.com', 'LE BANQUIER', 'Sébastien', 'Hetzel', '+32478678901', 'player', true, null);

-- Assign managers to clubs
INSERT INTO club_managers (id, club_id, user_id, assigned_by, notes) VALUES
    (gen_random_uuid(), '66666666-6666-6666-6666-666666666666', 'ffffffff-ffff-ffff-ffff-ffffffffffff', null, 'Manager of Poker One');
