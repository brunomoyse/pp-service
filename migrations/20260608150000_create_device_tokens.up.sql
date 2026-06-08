-- Expo push tokens per device, used to deliver push notifications (e.g.
-- achievement unlocks) to a player's physical devices.
CREATE TABLE device_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- The Expo push token (ExponentPushToken[...]). Globally unique: one
    -- physical device maps to exactly one row, reassigned to whichever user is
    -- currently signed in on that device.
    token TEXT UNIQUE NOT NULL,
    platform VARCHAR(10) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_device_tokens_user_id ON device_tokens(user_id);

CREATE TRIGGER trg_device_tokens_updated_at BEFORE UPDATE ON device_tokens
FOR EACH ROW EXECUTE PROCEDURE set_updated_at();
