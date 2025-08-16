-- Add points column to tournament_results table
ALTER TABLE tournament_results 
ADD COLUMN points INTEGER NOT NULL DEFAULT 0;