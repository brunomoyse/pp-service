-- Rollback: Restore the old check constraint
ALTER TABLE tournament_registrations 
DROP CONSTRAINT IF EXISTS tournament_registrations_status_check;

-- Convert any new status values back to old ones for rollback
UPDATE tournament_registrations 
SET status = 'pending', updated_at = NOW()
WHERE status IN ('registered', 'checked_in', 'seated');

UPDATE tournament_registrations 
SET status = 'eliminated', updated_at = NOW()
WHERE status = 'busted';

-- Restore old default value
ALTER TABLE tournament_registrations 
ALTER COLUMN status SET DEFAULT 'pending';

-- Restore old check constraint
ALTER TABLE tournament_registrations 
ADD CONSTRAINT tournament_registrations_status_check 
CHECK (status IN ('pending', 'waitlisted', 'cancelled', 'no_show', 'eliminated'));