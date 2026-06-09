-- Per-user notification preferences. No row means all defaults (everything
-- on); a row is created on first change (upsert from the app).
CREATE TABLE notification_preferences (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    tournament_reminders BOOLEAN NOT NULL DEFAULT TRUE,
    registration_updates BOOLEAN NOT NULL DEFAULT TRUE,
    seating_updates BOOLEAN NOT NULL DEFAULT TRUE,
    achievements BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER notification_preferences_set_updated_at
    BEFORE UPDATE ON notification_preferences
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();
