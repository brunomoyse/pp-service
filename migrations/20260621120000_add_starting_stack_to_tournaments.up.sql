-- Tournament-level default starting stack: the chip count a player receives on
-- their initial buy-in. NULL = not configured (managers can still enter chips
-- per entry). Distinct from tournament_registrations.starting_stack, which is a
-- per-player Day-2 carry-over for multi-day series.
ALTER TABLE tournaments ADD COLUMN starting_stack INTEGER;
