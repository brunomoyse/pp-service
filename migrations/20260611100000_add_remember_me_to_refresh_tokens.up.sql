-- Persist the "remember me" choice with the refresh token so cookie rotation
-- can preserve it. Previously /auth/refresh always re-issued a persistent
-- (Max-Age) cookie, silently upgrading session-only logins to remembered ones.
ALTER TABLE refresh_tokens ADD COLUMN remember_me BOOLEAN NOT NULL DEFAULT FALSE;

-- Tokens issued before this migration all behaved as persistent; keep live
-- sessions on that behavior rather than logging everyone out at next restart.
UPDATE refresh_tokens SET remember_me = TRUE WHERE revoked_at IS NULL;
