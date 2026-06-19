-- Redemption codes: hand a club a code that flips it onto a paid plan for a
-- fixed trial window (e.g. "3 months free Club"). This is the manual / promo
-- counterpart to the Mollie checkout — the owner mints codes, clubs redeem them
-- in the manager app, and the subscription-expiry sweep downgrades them back to
-- free once `subscription_expires_at` passes.
CREATE TABLE redemption_codes (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Stored normalized (trimmed, upper-case) so lookups are case-insensitive.
    code        TEXT NOT NULL UNIQUE,
    -- Target tier the code grants. Never 'free' — a code only ever upgrades.
    plan        TEXT NOT NULL DEFAULT 'club' CHECK (plan IN ('club', 'casino')),
    -- Length of the free trial granted on redemption.
    trial_days  INTEGER NOT NULL CHECK (trial_days > 0),
    -- Total redemptions allowed across all clubs. NULL = unlimited.
    max_uses    INTEGER CHECK (max_uses IS NULL OR max_uses > 0),
    used_count  INTEGER NOT NULL DEFAULT 0,
    -- When the code itself stops being redeemable (NULL = never expires).
    expires_at  TIMESTAMPTZ,
    -- Free-text reminder of what this code is for (admin-facing only).
    note        TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- One row per (code, club) redemption. The UNIQUE constraint makes a club's
-- second attempt at the same code a no-op error rather than a double-grant.
CREATE TABLE redemption_code_uses (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code_id     UUID NOT NULL REFERENCES redemption_codes(id) ON DELETE CASCADE,
    club_id     UUID NOT NULL REFERENCES clubs(id) ON DELETE CASCADE,
    redeemed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    redeemed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (code_id, club_id)
);

CREATE INDEX idx_redemption_code_uses_club ON redemption_code_uses (club_id);

CREATE TRIGGER trg_redemption_codes_updated_at
    BEFORE UPDATE ON redemption_codes
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- No seeded codes: admins mint them on demand from the manager app
-- (admin-only "Trial codes" screen → createRedemptionCode), then send the
-- generated code to a club to redeem in Settings.
