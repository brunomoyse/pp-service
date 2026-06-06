-- Identity & roster foundation.
--
-- `registered_player` is the club roster: one row per person a club has
-- registered, whether or not they are an onboarded app user. `app_user_id`
-- links the roster entry to a `users` row once the person joins and claims it
-- (nullable so clubs can register people who are not app users yet).
--
-- One app user maps to many registered_player rows (one per club) — that fan-out
-- is the cross-club profile. tournament_registrations/results keep their
-- existing `user_id` column during this transition and gain a nullable
-- `registered_player_id` that is backfilled and kept in sync by a trigger, so
-- no existing insert path needs to change.

CREATE TABLE registered_player (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    club_id       UUID NOT NULL REFERENCES clubs(id) ON DELETE CASCADE,
    display_name  TEXT NOT NULL,
    app_user_id   UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX registered_player_club_id_idx ON registered_player (club_id);
CREATE INDEX registered_player_app_user_id_idx ON registered_player (app_user_id);

-- At most one roster entry per (club, app user). Multiple NULL app_user_id rows
-- (non-users) are allowed per club.
CREATE UNIQUE INDEX registered_player_club_app_user_uniq
    ON registered_player (club_id, app_user_id)
    WHERE app_user_id IS NOT NULL;

CREATE TRIGGER trg_registered_player_updated_at
    BEFORE UPDATE ON registered_player
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- Add nullable roster FK to the operational tables (kept alongside user_id).
ALTER TABLE tournament_registrations
    ADD COLUMN registered_player_id UUID REFERENCES registered_player(id) ON DELETE SET NULL;
ALTER TABLE tournament_results
    ADD COLUMN registered_player_id UUID REFERENCES registered_player(id) ON DELETE SET NULL;

-- Backfill the roster from everyone who has ever registered or had a result.
INSERT INTO registered_player (club_id, display_name, app_user_id)
SELECT DISTINCT
    t.club_id,
    COALESCE(
        NULLIF(TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, '')), ''),
        u.username,
        u.email
    ),
    u.id
FROM (
    SELECT tournament_id, user_id FROM tournament_registrations
    UNION
    SELECT tournament_id, user_id FROM tournament_results
) src
JOIN tournaments t ON t.id = src.tournament_id
JOIN users u ON u.id = src.user_id
ON CONFLICT DO NOTHING;

-- Point existing registrations/results at their roster entry.
UPDATE tournament_registrations tr
SET registered_player_id = rp.id
FROM tournaments t, registered_player rp
WHERE tr.tournament_id = t.id
  AND rp.club_id = t.club_id
  AND rp.app_user_id = tr.user_id
  AND tr.registered_player_id IS NULL;

UPDATE tournament_results res
SET registered_player_id = rp.id
FROM tournaments t, registered_player rp
WHERE res.tournament_id = t.id
  AND rp.club_id = t.club_id
  AND rp.app_user_id = res.user_id
  AND res.registered_player_id IS NULL;

CREATE INDEX tournament_registrations_registered_player_id_idx
    ON tournament_registrations (registered_player_id);
CREATE INDEX tournament_results_registered_player_id_idx
    ON tournament_results (registered_player_id);

-- Keep the roster self-maintaining: on insert, find-or-create the roster entry
-- for (the tournament's club, the inserting user) and stamp registered_player_id.
-- Existing Rust insert paths need no changes.
CREATE OR REPLACE FUNCTION link_registered_player()
RETURNS TRIGGER AS $$
DECLARE
    v_club_id UUID;
    v_rp_id   UUID;
    v_name    TEXT;
BEGIN
    IF NEW.registered_player_id IS NOT NULL THEN
        RETURN NEW;
    END IF;

    SELECT club_id INTO v_club_id FROM tournaments WHERE id = NEW.tournament_id;
    IF v_club_id IS NULL THEN
        RETURN NEW;
    END IF;

    SELECT id INTO v_rp_id FROM registered_player
        WHERE club_id = v_club_id AND app_user_id = NEW.user_id;

    IF v_rp_id IS NULL THEN
        SELECT COALESCE(
            NULLIF(TRIM(COALESCE(first_name, '') || ' ' || COALESCE(last_name, '')), ''),
            username,
            email,
            'Unknown'
        )
        INTO v_name FROM users WHERE id = NEW.user_id;

        INSERT INTO registered_player (club_id, display_name, app_user_id)
            VALUES (v_club_id, COALESCE(v_name, 'Unknown'), NEW.user_id)
            RETURNING id INTO v_rp_id;
    END IF;

    NEW.registered_player_id := v_rp_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_link_registered_player_registrations
    BEFORE INSERT ON tournament_registrations
    FOR EACH ROW EXECUTE PROCEDURE link_registered_player();

CREATE TRIGGER trg_link_registered_player_results
    BEFORE INSERT ON tournament_results
    FOR EACH ROW EXECUTE PROCEDURE link_registered_player();
