CREATE TABLE tags (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug       TEXT NOT NULL UNIQUE,   -- e.g., "freezeout", "bounty"
    label      TEXT NOT NULL,          -- display name
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX tags_label_trgm_idx ON tags USING GIN (LOWER(label) gin_trgm_ops);

CREATE TRIGGER trg_tags_updated_at
    BEFORE UPDATE ON tags
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- Link table: many-to-many tournaments <-> tags
CREATE TABLE tournament_tags (
    tournament_id UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    tag_id        UUID NOT NULL REFERENCES tags(id)        ON DELETE CASCADE,
    PRIMARY KEY (tournament_id, tag_id)
);

-- For fast filtering all tournaments by a tag
CREATE INDEX tournament_tags_tag_id_idx ON tournament_tags (tag_id);