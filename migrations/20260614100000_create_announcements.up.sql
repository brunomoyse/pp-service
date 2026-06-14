-- Club / tournament / platform announcements authored by managers and admins,
-- broadcast to players as a push notification and kept as a persistent in-app
-- feed (this row is the source of truth — pushes themselves are ephemeral).
--
-- Three scopes decide the audience:
--   tournament -> app users registered (not cancelled/no_show) for the tournament
--   club       -> the club's roster app users (claimed, active club_player rows)
--   platform   -> every active player (admin-only)
--
-- title/body are author-written free text in a single language (no per-locale
-- copy): the push body is the authored text verbatim.
CREATE TABLE announcements (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scope         TEXT NOT NULL CHECK (scope IN ('tournament', 'club', 'platform')),
    -- Set for tournament/club scopes, NULL for platform.
    club_id       UUID REFERENCES clubs(id) ON DELETE CASCADE,
    -- Set for tournament scope only.
    tournament_id UUID REFERENCES tournaments(id) ON DELETE CASCADE,
    title         TEXT NOT NULL,
    body          TEXT NOT NULL,
    created_by    UUID REFERENCES users(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Scope/target coherence: tournament needs both ids, club needs club only,
    -- platform needs neither.
    CONSTRAINT announcements_scope_targets CHECK (
        (scope = 'tournament' AND club_id IS NOT NULL AND tournament_id IS NOT NULL)
        OR (scope = 'club' AND club_id IS NOT NULL AND tournament_id IS NULL)
        OR (scope = 'platform' AND club_id IS NULL AND tournament_id IS NULL)
    )
);

CREATE INDEX idx_announcements_club ON announcements (club_id);
CREATE INDEX idx_announcements_tournament ON announcements (tournament_id);
CREATE INDEX idx_announcements_created_at ON announcements (created_at DESC);

CREATE TRIGGER trg_announcements_updated_at
    BEFORE UPDATE ON announcements
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();
