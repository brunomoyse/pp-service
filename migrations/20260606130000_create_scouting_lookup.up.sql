-- Phase 8b: opponent lookup (SharkScope-style), consent-gated.
--
-- Only users who opted into the pool (user_privacy_settings.in_scouting_pool)
-- are discoverable. This table logs profile views so the server can enforce a
-- free-search quota (limited free lookups; unlimited only for Pro who are
-- themselves in the pool — reciprocity).

CREATE TABLE scouting_lookup (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    searcher_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    target_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX scouting_lookup_searcher_idx ON scouting_lookup (searcher_id, created_at DESC);
