-- Freemium tiering: each club is on one of three plans.
--   free   — "Home Game": single table, 1 active tournament, no recurring.
--   club   — €49/mo: unlimited tables & tournaments, recurring, staff.
--   casino — €149/mo: everything in club + cash games + rooms (sales-led).
-- Only `free` is gated; club/casino are unlimited on these dimensions.
ALTER TABLE clubs
    ADD COLUMN plan TEXT NOT NULL DEFAULT 'free'
        CHECK (plan IN ('free', 'club', 'casino')),
    ADD COLUMN subscription_status TEXT,
    ADD COLUMN subscription_expires_at TIMESTAMPTZ;

-- Grandfather every existing club to `club`: they all onboarded as real VAT
-- businesses before this feature and must not suddenly hit free-tier limits.
UPDATE clubs SET plan = 'club';
