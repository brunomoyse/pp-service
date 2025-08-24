-- Revert 'pending' statuses back to 'registered'
UPDATE tournament_registrations 
SET status = 'registered', updated_at = NOW()
WHERE status = 'pending';

-- Restore the original check constraint
ALTER TABLE tournament_registrations 
DROP CONSTRAINT IF EXISTS tournament_registrations_status_check;

ALTER TABLE tournament_registrations 
ADD CONSTRAINT tournament_registrations_status_check 
CHECK (status IN ('registered', 'waitlisted', 'cancelled', 'no_show'));