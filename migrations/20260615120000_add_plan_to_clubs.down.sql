ALTER TABLE clubs
    DROP COLUMN IF EXISTS subscription_expires_at,
    DROP COLUMN IF EXISTS subscription_status,
    DROP COLUMN IF EXISTS plan;
