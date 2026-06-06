-- Phase 5c: seasons, season pass (XP track), weekly quest completions.
-- Engagement is FREE and earned-only (constraint G1): season-pass XP derives from
-- attendance (`check_in`) and quest claims; the premium reward track is a gift
-- entitlement (`is_premium`), never a purchasable randomised item.

-- A club's competitive season — a named time window.
CREATE TABLE season (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    club_id    UUID NOT NULL REFERENCES clubs(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    starts_at  TIMESTAMPTZ NOT NULL,
    ends_at    TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (ends_at > starts_at)
);

CREATE INDEX season_club_idx ON season (club_id, starts_at DESC);

-- A player's pass for a season. XP itself is derived (not stored); this row only
-- records the gifted premium track and lets us list participants.
CREATE TABLE season_pass (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    season_id    UUID NOT NULL REFERENCES season(id) ON DELETE CASCADE,
    app_user_id  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    is_premium   BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (season_id, app_user_id)
);

CREATE TRIGGER trg_season_pass_updated_at
    BEFORE UPDATE ON season_pass
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- A claimed weekly quest. The quest catalog and weekly rotation live in code
-- (deterministic from the ISO week); only completions are persisted, idempotent
-- per (user, quest, week).
CREATE TABLE quest_completion (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_user_id  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    quest_code   TEXT NOT NULL,
    week_start   DATE NOT NULL,
    xp_awarded   INT NOT NULL,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (app_user_id, quest_code, week_start)
);

CREATE INDEX quest_completion_user_idx ON quest_completion (app_user_id, completed_at DESC);
