-- Update the check constraint to use new status values
ALTER TABLE tournament_registrations 
DROP CONSTRAINT IF EXISTS tournament_registrations_status_check;

-- Convert existing data to new status values (required for constraint to succeed)
UPDATE tournament_registrations 
SET status = 'registered', updated_at = NOW()
WHERE status = 'pending';

UPDATE tournament_registrations 
SET status = 'busted', updated_at = NOW()
WHERE status = 'eliminated';

-- Update default value to use new status
ALTER TABLE tournament_registrations 
ALTER COLUMN status SET DEFAULT 'registered';

-- Add new check constraint with updated status values
ALTER TABLE tournament_registrations 
ADD CONSTRAINT tournament_registrations_status_check 
CHECK (status IN ('registered', 'checked_in', 'seated', 'busted', 'waitlisted', 'cancelled', 'no_show'));