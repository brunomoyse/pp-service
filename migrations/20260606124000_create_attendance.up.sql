-- First-class check-in + attendance streaks.
--
-- check_in records the dopamine event (one per player per tournament), separate
-- from the registration status flow. attendance_streak tracks the running streak
-- with a small number of "freezes" that forgive a missed week.

CREATE TABLE check_in (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_user_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tournament_id UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    club_id       UUID NOT NULL REFERENCES clubs(id) ON DELETE CASCADE,
    checked_in_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (app_user_id, tournament_id)
);

CREATE INDEX check_in_user_idx ON check_in (app_user_id, checked_in_at DESC);
CREATE INDEX check_in_tournament_idx ON check_in (tournament_id);

CREATE TABLE attendance_streak (
    app_user_id       UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    current_streak    INT NOT NULL DEFAULT 0,
    longest_streak    INT NOT NULL DEFAULT 0,
    last_check_in_at  TIMESTAMPTZ,
    freezes_available INT NOT NULL DEFAULT 2,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER trg_attendance_streak_updated_at
    BEFORE UPDATE ON attendance_streak
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();
