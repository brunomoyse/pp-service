-- Track account activity (login / token refresh) so the data-retention job can
-- distinguish dormant accounts from active-but-quiet ones. Nullable; backfilled
-- from updated_at for existing rows, then maintained by the auth flow.
ALTER TABLE users ADD COLUMN last_seen_at TIMESTAMPTZ;

UPDATE users SET last_seen_at = updated_at WHERE last_seen_at IS NULL;

CREATE INDEX users_last_seen_idx ON users (last_seen_at);
