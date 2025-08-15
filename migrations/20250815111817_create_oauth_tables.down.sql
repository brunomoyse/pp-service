-- Drop triggers
DROP TRIGGER trg_oauth_clients_updated_at ON oauth_clients;

-- Drop indexes
DROP INDEX oauth_refresh_tokens_access_token_id_idx;
DROP INDEX oauth_refresh_tokens_token_idx;
DROP INDEX oauth_access_tokens_expires_at_idx;
DROP INDEX oauth_access_tokens_user_id_idx;
DROP INDEX oauth_access_tokens_token_idx;
DROP INDEX oauth_authorization_codes_expires_at_idx;
DROP INDEX oauth_authorization_codes_user_id_idx;
DROP INDEX oauth_authorization_codes_code_idx;
DROP INDEX oauth_clients_client_id_idx;

-- Drop tables
DROP TABLE oauth_refresh_tokens;
DROP TABLE oauth_access_tokens;
DROP TABLE oauth_authorization_codes;
DROP TABLE oauth_clients;

-- Remove password field from users table
ALTER TABLE users DROP COLUMN password_hash;