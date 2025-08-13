CREATE TABLE users (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email        TEXT NOT NULL UNIQUE,
    username     TEXT UNIQUE,
    first_name   TEXT NOT NULL,
    last_name    TEXT NOT NULL,
    phone        TEXT,
    is_active    BOOLEAN NOT NULL DEFAULT TRUE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Useful indexes
CREATE INDEX users_email_idx ON users (email);
CREATE INDEX users_username_idx ON users (username);
CREATE INDEX users_username_trgm_idx ON users USING GIN (LOWER(username) gin_trgm_ops);

-- updated_at trigger
CREATE TRIGGER trg_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();