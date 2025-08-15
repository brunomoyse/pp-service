-- Add password field to users table for custom OAuth
ALTER TABLE users ADD COLUMN password_hash TEXT;

-- OAuth clients table for managing applications that can authenticate via your OAuth server
CREATE TABLE oauth_clients (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_id TEXT NOT NULL UNIQUE,
    client_secret TEXT NOT NULL,
    name TEXT NOT NULL,
    redirect_uris TEXT[] NOT NULL,
    scopes TEXT[] NOT NULL DEFAULT ARRAY['read'],
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- OAuth authorization codes table for the authorization code flow
CREATE TABLE oauth_authorization_codes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code TEXT NOT NULL UNIQUE,
    client_id UUID NOT NULL REFERENCES oauth_clients(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    redirect_uri TEXT NOT NULL,
    scopes TEXT[] NOT NULL,
    challenge TEXT, -- PKCE code challenge
    challenge_method TEXT, -- PKCE code challenge method
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- OAuth access tokens table
CREATE TABLE oauth_access_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token TEXT NOT NULL UNIQUE,
    client_id UUID NOT NULL REFERENCES oauth_clients(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scopes TEXT[] NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- OAuth refresh tokens table
CREATE TABLE oauth_refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token TEXT NOT NULL UNIQUE,
    access_token_id UUID NOT NULL REFERENCES oauth_access_tokens(id) ON DELETE CASCADE,
    client_id UUID NOT NULL REFERENCES oauth_clients(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX oauth_clients_client_id_idx ON oauth_clients (client_id);
CREATE INDEX oauth_authorization_codes_code_idx ON oauth_authorization_codes (code);
CREATE INDEX oauth_authorization_codes_user_id_idx ON oauth_authorization_codes (user_id);
CREATE INDEX oauth_authorization_codes_expires_at_idx ON oauth_authorization_codes (expires_at);
CREATE INDEX oauth_access_tokens_token_idx ON oauth_access_tokens (token);
CREATE INDEX oauth_access_tokens_user_id_idx ON oauth_access_tokens (user_id);
CREATE INDEX oauth_access_tokens_expires_at_idx ON oauth_access_tokens (expires_at);
CREATE INDEX oauth_refresh_tokens_token_idx ON oauth_refresh_tokens (token);
CREATE INDEX oauth_refresh_tokens_access_token_id_idx ON oauth_refresh_tokens (access_token_id);

-- Triggers for updated_at
CREATE TRIGGER trg_oauth_clients_updated_at
    BEFORE UPDATE ON oauth_clients
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();