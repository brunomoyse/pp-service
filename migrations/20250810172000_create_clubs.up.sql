CREATE TABLE clubs (
   id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
   name        TEXT NOT NULL,
   city        TEXT,
   country     TEXT DEFAULT 'BE',
   created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
   updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Fast lookup by name (case-insensitive) and city
CREATE INDEX clubs_name_trgm_idx ON clubs USING GIN (LOWER(name) gin_trgm_ops);
CREATE INDEX clubs_city_idx      ON clubs (city);

-- updated_at trigger
CREATE TRIGGER trg_clubs_updated_at
    BEFORE UPDATE ON clubs
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();