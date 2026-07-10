-- HUD-style color tag on a player note: a quick visual bucket the author
-- assigns to a player (red = tough reg, blue = fish, and so on). Independent of
-- the style quadrant; nullable (no color = uncategorised).
ALTER TABLE player_note
    ADD COLUMN color TEXT CHECK (color IN ('red', 'orange', 'yellow', 'green', 'blue', 'purple'));
