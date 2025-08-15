-- Add role column to users table
ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'user';

-- Create index for role lookups
CREATE INDEX users_role_idx ON users (role);

-- Add check constraint for valid roles
ALTER TABLE users ADD CONSTRAINT users_role_check CHECK (role IN ('user', 'admin', 'moderator'));