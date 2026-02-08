-- Cleanup: delete all seeded data in child-first FK order
DELETE FROM table_seat_assignments;
DELETE FROM tournament_results;
DELETE FROM tournament_registrations;
DELETE FROM tournament_tags;
DELETE FROM tournament_structures;
DELETE FROM tournament_clocks;
DELETE FROM player_deals;
DELETE FROM tournament_entries;
DELETE FROM tournament_payouts;
DELETE FROM tournament_table_assignments;
DELETE FROM payout_templates;
DELETE FROM tags;
DELETE FROM tournaments;
DELETE FROM club_managers;
DELETE FROM club_tables;
DELETE FROM users;
DELETE FROM clubs;
