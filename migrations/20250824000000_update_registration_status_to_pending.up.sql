-- Update existing 'registered' statuses to 'pending'
UPDATE tournament_registrations 
SET status = 'pending', updated_at = NOW()
WHERE status = 'registered';

-- Update the check constraint to use 'pending' instead of 'registered'
ALTER TABLE tournament_registrations 
DROP CONSTRAINT IF EXISTS tournament_registrations_status_check;

ALTER TABLE tournament_registrations 
ADD CONSTRAINT tournament_registrations_status_check 
CHECK (status IN ('pending', 'waitlisted', 'cancelled', 'no_show'));