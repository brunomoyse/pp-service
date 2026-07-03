-- Full reset for e2e / local seeding.
--
-- Truncating the four seed *roots* (clubs, users, tournaments, tags) with
-- CASCADE clears every club-, user- and tournament-scoped child table
-- transitively via their ON DELETE CASCADE foreign keys: registrations,
-- results, entries, seat + table assignments, clocks + clock events, the
-- activity log, bounties, deals, predictions, drink wallets + ledger +
-- redemptions, series, announcements, player achievements, device tokens,
-- oauth/refresh/reset tokens, notification prefs, and so on.
--
-- This is deliberately CASCADE rather than a hand-maintained child-first
-- DELETE list: the old list silently rotted every time a new table was added
-- (that is why a clean reset used to require recreating the whole container).
-- CASCADE stays correct with zero maintenance.
--
-- Global, migration-seeded catalogs are NOT club-scoped and are intentionally
-- left intact: achievements definitions, cosmetic_item, oauth_clients, etc.
--
-- Re-inserting a club re-fires the per-club template trigger
-- (seed_club_default_templates), so blind-structure and payout templates come
-- back automatically on the next seed — the create-tournament flow always has
-- a template to pick.
--
-- RESTART IDENTITY resets any serial/identity sequences so IDs are stable
-- across runs.
TRUNCATE TABLE
  clubs,
  users,
  tournaments,
  tags
RESTART IDENTITY CASCADE;
