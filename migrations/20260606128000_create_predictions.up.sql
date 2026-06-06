-- Phase 7b: Prediction Points (PP) — the SECOND sealed economy.
--
-- Constraint G2 (two sealed economies): PP is EARNED-ONLY. There is no euro
-- entry, ever — every ledger reason is a fantasy/engagement action. These tables
-- carry NO foreign key to the euro cosmetic tables, and the euro tables carry
-- none to these. PP is a free fantasy currency: never convertible to/from euros,
-- and the product copy calls it "prediction/fantasy", never "betting".
--
-- `ref_id` is a bare UUID (NOT a foreign key) on purpose: it loosely references a
-- prediction/tournament without creating a transaction path that could be used
-- to argue the economies touch.

CREATE TABLE prediction_point_ledger (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Signed: positive credits (earned/payout/seed), negative debits (stake).
    delta       INT NOT NULL,
    reason      TEXT NOT NULL CHECK (reason IN ('earned', 'prediction_stake', 'prediction_payout', 'seed')),
    ref_id      UUID,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX ppl_user_idx ON prediction_point_ledger (app_user_id, created_at DESC);

CREATE TABLE prediction_entry (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_user_id              UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tournament_id            UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    predicted_winner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    stake_points             INT NOT NULL CHECK (stake_points > 0),
    status                   TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'won', 'lost')),
    payout_points            INT NOT NULL DEFAULT 0,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at              TIMESTAMPTZ,
    UNIQUE (app_user_id, tournament_id)
);

CREATE INDEX prediction_entry_user_idx ON prediction_entry (app_user_id);
CREATE INDEX prediction_entry_tournament_idx ON prediction_entry (tournament_id, status);
