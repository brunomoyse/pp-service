-- Phase 8a: per-user privacy/consent settings (G3/G4/G5/G6).
--
-- Two GRANULAR, independent consents, both defaulting FALSE (G4 / GDPR Art.7(4)
-- — no bundling, no pre-ticked boxes):
--   * in_scouting_pool — consent to be DISCOVERABLE in opponent lookup and to
--     expose tournament performance stats to searchers.
--   * share_named_pl   — the stricter, separate consent to attach identifiable
--     profit/loss (the euro figure) to that profile.
--
-- A pseudonymised "Anonymous #N" is still personal data (G5), so individual
-- stats are surfaced ONLY for users who opted into the pool. The whole feature
-- ships behind FEATURE_PUBLIC_STATS and must not launch before legal sign-off (G6).

CREATE TABLE user_privacy_settings (
    app_user_id      UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    share_named_pl   BOOLEAN NOT NULL DEFAULT FALSE,
    in_scouting_pool BOOLEAN NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER trg_user_privacy_settings_updated_at
    BEFORE UPDATE ON user_privacy_settings
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();
