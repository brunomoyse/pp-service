-- Pro entitlements.
--
-- A user is "Pro" while they hold at least one active, unexpired entitlement.
-- For launch, entitlements are granted by a club to its regulars (the B2B tie-in)
-- or manually; the `source` column leaves room for app-store purchases later
-- without changing the read model.

CREATE TABLE pro_entitlement (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_user_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source             TEXT NOT NULL DEFAULT 'club_gift'
                         CHECK (source IN ('club_gift', 'purchase', 'manual')),
    granted_by_club_id UUID REFERENCES clubs(id) ON DELETE SET NULL,
    granted_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    starts_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at         TIMESTAMPTZ,
    status             TEXT NOT NULL DEFAULT 'active'
                         CHECK (status IN ('active', 'revoked')),
    notes              TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX pro_entitlement_app_user_idx ON pro_entitlement (app_user_id);
CREATE INDEX pro_entitlement_active_idx
    ON pro_entitlement (app_user_id)
    WHERE status = 'active';

CREATE TRIGGER trg_pro_entitlement_updated_at
    BEFORE UPDATE ON pro_entitlement
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();
