-- Remove OAuth provider fields from users table
DROP INDEX users_oauth_provider_idx;

ALTER TABLE users 
DROP COLUMN oauth_provider,
DROP COLUMN oauth_provider_id,
DROP COLUMN avatar_url;

-- Restore last_name not null constraint
ALTER TABLE users ALTER COLUMN last_name SET NOT NULL;