-- Add early bird bonus chips column to tournaments
-- This allows tournaments to grant extra chips to players who check in before the tournament starts
ALTER TABLE tournaments ADD COLUMN early_bird_bonus_chips INTEGER DEFAULT NULL;
