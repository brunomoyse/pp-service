-- Split the club roster identity into first_name / last_name while keeping
-- display_name as the canonical, always-present label (maintained as
-- "first last") so every existing consumer (leaderboards, results, seating,
-- GraphQL ClubPlayer.display_name) keeps working unchanged.
ALTER TABLE club_player
    ADD COLUMN first_name TEXT,
    ADD COLUMN last_name TEXT;

-- Best-effort backfill from display_name: last whitespace-delimited token is the
-- family name, everything before it the given name(s). Single-token names keep
-- the whole value as first_name with an empty last_name.
UPDATE club_player
SET
    last_name = CASE
        WHEN trim(display_name) ~ '\s'
        THEN substring(trim(display_name) from '(\S+)\s*$')
        ELSE ''
    END,
    first_name = CASE
        WHEN trim(display_name) ~ '\s'
        THEN trim(regexp_replace(trim(display_name), '\s+\S+\s*$', ''))
        ELSE trim(display_name)
    END
WHERE first_name IS NULL;

-- Order rosters by family name by default.
CREATE INDEX club_player_last_name_idx ON club_player (last_name, first_name);
