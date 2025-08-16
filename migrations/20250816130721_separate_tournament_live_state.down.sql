-- Drop tournament_state table
DROP TABLE IF EXISTS tournament_state;

-- Add back live state columns to tournaments table
ALTER TABLE tournaments 
ADD COLUMN current_level INTEGER,
ADD COLUMN players_remaining INTEGER;