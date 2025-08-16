-- Drop the triggers
DROP TRIGGER IF EXISTS auto_create_clock_insert_trigger ON tournaments;
DROP TRIGGER IF EXISTS auto_create_clock_trigger ON tournaments;

-- Drop the trigger function
DROP FUNCTION IF EXISTS auto_create_tournament_clock();