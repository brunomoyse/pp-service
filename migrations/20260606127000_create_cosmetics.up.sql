-- Phase 7a: euro cosmetics — the FIRST sealed economy.
--
-- Constraint G1 (no paid randomness): every cosmetic is a named, previewable,
-- fixed-price item. There is NO loot-box / random-acquisition path: a purchase
-- maps 1:1 to a specific `cosmetic_item` at its listed price.
--
-- Constraint G2 (sealed economies): these euro tables hold NO reference of any
-- kind to the Prediction-Points tables (added in 7b). The only foreign keys are
-- to neutral tables (users, clubs, cosmetic_item). An automated test asserts no
-- FK path ever connects this economy to the PP ledger.

CREATE TABLE cosmetic_item (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code        TEXT NOT NULL UNIQUE,
    kind        TEXT NOT NULL CHECK (kind IN ('card_back', 'avatar_frame', 'theme', 'badge')),
    name        TEXT NOT NULL,
    description TEXT,
    -- Euro price in integer cents. Fixed and shown before purchase (G1).
    price_cents INT NOT NULL CHECK (price_cents >= 0),
    -- Asset key the client maps to a preview (preview-before-buy, G1).
    preview_ref TEXT NOT NULL,
    -- Optional club branding (club-branded cosmetics, spec §4.3).
    club_id     UUID REFERENCES clubs(id) ON DELETE CASCADE,
    active      BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX cosmetic_item_kind_idx ON cosmetic_item (kind) WHERE active;

CREATE TABLE user_cosmetic (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    cosmetic_item_id UUID NOT NULL REFERENCES cosmetic_item(id) ON DELETE CASCADE,
    source           TEXT NOT NULL CHECK (source IN ('purchase', 'club_gift', 'reward')),
    equipped         BOOLEAN NOT NULL DEFAULT FALSE,
    acquired_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (app_user_id, cosmetic_item_id)
);

CREATE INDEX user_cosmetic_user_idx ON user_cosmetic (app_user_id);

-- Euro ledger. The presence of `currency`/`price_cents` (and the ABSENCE of any
-- points column) marks this as the euro economy.
CREATE TABLE cosmetic_purchase (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    cosmetic_item_id UUID NOT NULL REFERENCES cosmetic_item(id) ON DELETE RESTRICT,
    price_cents      INT NOT NULL,
    currency         TEXT NOT NULL DEFAULT 'EUR',
    purchased_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX cosmetic_purchase_user_idx ON cosmetic_purchase (app_user_id, purchased_at DESC);

-- Deterministic, named, previewable catalog (G1).
INSERT INTO cosmetic_item (code, kind, name, description, price_cents, preview_ref) VALUES
    ('card_back_classic_gold', 'card_back', 'Classic Gold', 'A warm gold-foil card back.', 299, 'card_back/classic_gold'),
    ('card_back_midnight', 'card_back', 'Midnight', 'Deep navy with a subtle starfield.', 299, 'card_back/midnight'),
    ('avatar_frame_champion', 'avatar_frame', 'Champion Frame', 'A laurel frame for your avatar.', 399, 'avatar_frame/champion'),
    ('theme_emerald', 'theme', 'Emerald Felt', 'An emerald table theme.', 499, 'theme/emerald'),
    ('badge_high_roller', 'badge', 'High Roller', 'A flashy high-roller badge.', 199, 'badge/high_roller');
