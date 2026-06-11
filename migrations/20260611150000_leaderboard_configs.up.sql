-- Configurable leaderboards / leagues.
--
-- A league = (club, period, scoring formula). Points are computed on read in
-- Rust from `formula_params`; nothing is materialized here. The legacy
-- leaderboard path (no config) keeps using the stored `tournament_results.points`.

-- 1. League definitions.
CREATE TABLE leaderboard_configs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    club_id         UUID NOT NULL REFERENCES clubs(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    -- Serialized ScoringFormula (base_points, field_multiplier, buyin_multiplier,
    -- position_curve, min_players, cap, count_best_n).
    formula_params  JSONB NOT NULL,
    -- all_in_period: every club tournament whose start_time is in the league's
    -- period. tagged: only tournaments explicitly linked via
    -- tournaments.leaderboard_config_id.
    membership_mode TEXT NOT NULL DEFAULT 'all_in_period'
        CHECK (membership_mode IN ('all_in_period', 'tagged')),
    period_start    TIMESTAMPTZ,
    period_end      TIMESTAMPTZ,
    is_default      BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_leaderboard_configs_club ON leaderboard_configs (club_id);

-- 2. Audited manual point adjustments (Kholdem "manual points", always with a reason).
CREATE TABLE leaderboard_adjustments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    config_id       UUID NOT NULL REFERENCES leaderboard_configs(id) ON DELETE CASCADE,
    club_player_id  UUID NOT NULL REFERENCES club_player(id),
    points_delta    INTEGER NOT NULL,
    reason          TEXT NOT NULL,
    created_by      UUID REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_leaderboard_adjustments_config ON leaderboard_adjustments (config_id);

-- 3. Optional per-tournament league tag (feeds `tagged` leagues).
ALTER TABLE tournaments
    ADD COLUMN leaderboard_config_id UUID REFERENCES leaderboard_configs(id) ON DELETE SET NULL;
