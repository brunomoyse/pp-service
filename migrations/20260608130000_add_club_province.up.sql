-- Province-based leaderboards: a club's province is derived from its postal
-- code, not tagged by hand. Belgian 4-digit postal codes map deterministically
-- to the 10 provinces + Brussels-Capital, so `province` is a STORED generated
-- column computed from `postal_code` and stays consistent automatically.

ALTER TABLE clubs ADD COLUMN postal_code TEXT;

-- Deterministic, immutable mapping from a Belgian postal code to a province
-- slug. Returns NULL for missing/malformed codes. Slugs are stable i18n keys
-- (localized client-side); they are NOT meant to be displayed raw.
CREATE OR REPLACE FUNCTION province_from_postal_code(pc TEXT)
RETURNS TEXT
LANGUAGE sql
IMMUTABLE
AS $$
    SELECT CASE
        WHEN pc IS NULL OR btrim(pc) !~ '^[0-9]{4}$' THEN NULL
        WHEN btrim(pc)::int BETWEEN 1000 AND 1299 THEN 'brussels'
        WHEN btrim(pc)::int BETWEEN 1300 AND 1499 THEN 'walloon-brabant'
        WHEN btrim(pc)::int BETWEEN 1500 AND 1999 THEN 'flemish-brabant'
        WHEN btrim(pc)::int BETWEEN 2000 AND 2999 THEN 'antwerp'
        WHEN btrim(pc)::int BETWEEN 3000 AND 3499 THEN 'flemish-brabant'
        WHEN btrim(pc)::int BETWEEN 3500 AND 3999 THEN 'limburg'
        WHEN btrim(pc)::int BETWEEN 4000 AND 4999 THEN 'liege'
        WHEN btrim(pc)::int BETWEEN 5000 AND 5999 THEN 'namur'
        WHEN btrim(pc)::int BETWEEN 6000 AND 6599 THEN 'hainaut'
        WHEN btrim(pc)::int BETWEEN 6600 AND 6999 THEN 'luxembourg'
        WHEN btrim(pc)::int BETWEEN 7000 AND 7999 THEN 'hainaut'
        WHEN btrim(pc)::int BETWEEN 8000 AND 8999 THEN 'west-flanders'
        WHEN btrim(pc)::int BETWEEN 9000 AND 9999 THEN 'east-flanders'
        ELSE NULL
    END
$$;

ALTER TABLE clubs ADD COLUMN province TEXT
    GENERATED ALWAYS AS (province_from_postal_code(postal_code)) STORED;

CREATE INDEX clubs_province_idx ON clubs (province);
