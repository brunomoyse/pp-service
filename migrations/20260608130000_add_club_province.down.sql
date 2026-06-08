DROP INDEX IF EXISTS clubs_province_idx;
ALTER TABLE clubs DROP COLUMN IF EXISTS province;
DROP FUNCTION IF EXISTS province_from_postal_code(TEXT);
ALTER TABLE clubs DROP COLUMN IF EXISTS postal_code;
