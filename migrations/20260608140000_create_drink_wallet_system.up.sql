-- Drink-voucher wallet system.
--
-- Players accrue drink credits (e.g. on tournament registration) and redeem them
-- at the club bar. The wallet is anchored to a `registered_player` (account-independent)
-- or is a bearer wallet (no owner). Balance lives in an append-only ledger; the
-- `balance` column caches SUM(delta). Any presentation method (printed QR card,
-- manual number, future Apple Wallet pass) is a credential pointing at a wallet.
--
-- Drinks are an integer count, not money. Redemption is server-authoritative.

-- A club-scoped point of redemption (a bar terminal / till). The redeem mutation
-- requires a trusted bar-station identity, modelled here as a row a club manager
-- operates against (not player self-service).
CREATE TABLE bar_station (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    club_id     UUID NOT NULL REFERENCES clubs(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX bar_station_club_id_idx ON bar_station (club_id);

CREATE TRIGGER trg_bar_station_updated_at
    BEFORE UPDATE ON bar_station
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- One drink wallet per person. `registered_player_id` NULL => bearer/anonymous wallet.
-- `balance` is the cached SUM(delta) of the ledger and may never go negative.
CREATE TABLE drink_wallet (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    registered_player_id UUID UNIQUE REFERENCES registered_player(id) ON DELETE SET NULL,
    club_id              UUID NOT NULL REFERENCES clubs(id) ON DELETE CASCADE,
    balance              INTEGER NOT NULL DEFAULT 0 CHECK (balance >= 0),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX drink_wallet_club_id_idx ON drink_wallet (club_id);
CREATE INDEX drink_wallet_registered_player_id_idx ON drink_wallet (registered_player_id);

CREATE TRIGGER trg_drink_wallet_updated_at
    BEFORE UPDATE ON drink_wallet
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- One row per drink served at the bar. The UNIQUE(wallet_id, idempotency_key)
-- constraint stops a double-scan / retry from double-debiting.
CREATE TABLE drink_redemption (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id       UUID NOT NULL REFERENCES drink_wallet(id) ON DELETE CASCADE,
    bar_station_id  UUID NOT NULL REFERENCES bar_station(id),
    drink_type      TEXT,
    idempotency_key TEXT NOT NULL,
    created_by      UUID REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (wallet_id, idempotency_key)
);

CREATE INDEX drink_redemption_wallet_id_idx ON drink_redemption (wallet_id);

-- Append-only ledger. Never UPDATE/DELETE a row. balance = SUM(delta).
-- A positive entry is a "lot" of credits (optionally carrying an expiry); negative
-- entries are redemptions, expiries, negative adjustments, or transfers out.
-- `source_ledger_entry_id` pins an `expiry` entry to the exact lot it expired, which
-- keeps the nightly expiry job re-run safe (a lot can never be expired twice).
CREATE TABLE drink_ledger_entry (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id             UUID NOT NULL REFERENCES drink_wallet(id) ON DELETE CASCADE,
    delta                 INTEGER NOT NULL,
    reason                TEXT NOT NULL CHECK (reason IN (
                              'tournament_topup', 'bar_redemption', 'expiry', 'adjustment', 'transfer'
                          )),
    tournament_id         UUID REFERENCES tournaments(id) ON DELETE SET NULL,
    expires_at            TIMESTAMPTZ,
    redemption_id         UUID REFERENCES drink_redemption(id) ON DELETE SET NULL,
    source_ledger_entry_id UUID REFERENCES drink_ledger_entry(id) ON DELETE SET NULL,
    transfer_id           UUID,
    created_by            UUID REFERENCES users(id),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX drink_ledger_entry_wallet_id_idx ON drink_ledger_entry (wallet_id);
CREATE INDEX drink_ledger_entry_wallet_created_idx ON drink_ledger_entry (wallet_id, created_at, id);
-- Drives the nightly expiry job: only positive lots that carry an expiry matter.
CREATE INDEX drink_ledger_entry_expiry_idx
    ON drink_ledger_entry (expires_at)
    WHERE expires_at IS NOT NULL AND delta > 0;

-- How a wallet is reached. A printed card starts life with status='printed' and a
-- NULL wallet_id (unassigned); activation binds it to a wallet and flips it to
-- 'active'. We store only the SHA-256 of the QR secret; the raw secret lives only
-- in the QR. Each credential is independently revocable.
CREATE TABLE drink_wallet_credential (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id  UUID REFERENCES drink_wallet(id) ON DELETE CASCADE,
    type       TEXT NOT NULL CHECK (type IN ('apple_pass', 'printed_card', 'manual_number')),
    token_hash BYTEA NOT NULL,
    status     TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('printed', 'active', 'revoked', 'consumed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (type, token_hash)
);

CREATE INDEX drink_wallet_credential_wallet_id_idx ON drink_wallet_credential (wallet_id);

CREATE TRIGGER trg_drink_wallet_credential_updated_at
    BEFORE UPDATE ON drink_wallet_credential
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();
