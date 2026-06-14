-- Per-user toggle for club/tournament/platform announcement pushes. Defaults on
-- so existing users keep receiving them; the in-app feed is unaffected by this.
ALTER TABLE notification_preferences
    ADD COLUMN announcements BOOLEAN NOT NULL DEFAULT TRUE;
