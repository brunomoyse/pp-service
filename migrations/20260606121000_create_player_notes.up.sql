-- Private opponent notes (the PokerTracker layer).
--
-- Author-only visibility: every read MUST filter by author_app_user_id. Notes
-- reference the club roster (registered_player), never a retyped name, so they
-- work on non-app-users too while minimising duplicated personal data.
--
-- One note "document" per (author, subject); structured tags/tells and showdown
-- observations hang off it.

CREATE TABLE player_note (
    id                           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    author_app_user_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    subject_registered_player_id UUID NOT NULL REFERENCES registered_player(id) ON DELETE CASCADE,
    body                         TEXT NOT NULL DEFAULT '',
    -- Player-style quadrant: 'TAG' | 'LAG' | 'TP' (tight-passive) | 'LP' (loose-passive)
    style                        TEXT CHECK (style IN ('TAG', 'LAG', 'TP', 'LP')),
    created_at                   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (author_app_user_id, subject_registered_player_id)
);

CREATE INDEX player_note_author_idx ON player_note (author_app_user_id);
CREATE INDEX player_note_subject_idx ON player_note (subject_registered_player_id);

CREATE TRIGGER trg_player_note_updated_at
    BEFORE UPDATE ON player_note
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- Structured quick tags and live tells.
CREATE TABLE player_note_tag (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    note_id     UUID NOT NULL REFERENCES player_note(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL DEFAULT 'tag' CHECK (kind IN ('tag', 'tell')),
    tag         TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (note_id, kind, tag)
);

CREATE INDEX player_note_tag_note_idx ON player_note_tag (note_id);

-- Hands seen at showdown (the live-only differentiator).
CREATE TABLE showdown_observation (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    note_id       UUID NOT NULL REFERENCES player_note(id) ON DELETE CASCADE,
    tournament_id UUID REFERENCES tournaments(id) ON DELETE SET NULL,
    description   TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX showdown_observation_note_idx ON showdown_observation (note_id);
