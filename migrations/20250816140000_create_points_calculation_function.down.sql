-- Drop the trigger and functions created for points calculation

DROP TRIGGER IF EXISTS tournament_status_points_trigger ON tournaments;
DROP FUNCTION IF EXISTS trigger_calculate_points();
DROP FUNCTION IF EXISTS calculate_tournament_points(UUID);
DROP FUNCTION IF EXISTS recalculate_all_tournament_points(UUID);