-- Drop the trigger
DROP TRIGGER IF EXISTS auto_create_structure_trigger ON tournament_clocks;

-- Drop the trigger function  
DROP FUNCTION IF EXISTS auto_create_tournament_structure();