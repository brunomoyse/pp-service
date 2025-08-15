-- Add OAuth provider fields to users table
ALTER TABLE users 
ADD COLUMN oauth_provider TEXT,
ADD COLUMN oauth_provider_id TEXT,
ADD COLUMN avatar_url TEXT;

-- Index for OAuth provider lookups
CREATE INDEX users_oauth_provider_idx ON users (oauth_provider, oauth_provider_id);

-- Make last_name nullable as some OAuth providers don't provide it
ALTER TABLE users ALTER COLUMN last_name DROP NOT NULL;