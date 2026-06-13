-- Prevent more than one 'initial' buy-in per roster player per tournament.
-- Rebuys / re-entries / add-ons legitimately produce multiple rows, so this is a
-- PARTIAL unique index scoped to entry_type = 'initial'. A duplicate initial
-- would otherwise inflate the prize pool (recalculate_prize_pool_from_entries
-- sums every funding entry).

-- Forward-safe: collapse any pre-existing duplicate initials first (keep the
-- earliest by created_at, then id) so the index builds on live data without
-- aborting startup. The surplus rows are exactly the over-counted buy-ins this
-- index is meant to prevent; deleting them re-fires the prize-pool recalc trigger.
DELETE FROM tournament_entries te
USING (
    SELECT id,
           ROW_NUMBER() OVER (
               PARTITION BY tournament_id, club_player_id
               ORDER BY created_at, id
           ) AS rn
    FROM tournament_entries
    WHERE entry_type = 'initial'
) dups
WHERE te.id = dups.id AND dups.rn > 1;

CREATE UNIQUE INDEX uniq_initial_entry_per_player
    ON tournament_entries (tournament_id, club_player_id)
    WHERE entry_type = 'initial';
