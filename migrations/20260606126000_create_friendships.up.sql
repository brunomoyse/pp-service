-- Phase 6b: friendships + mutual flames.
-- A friendship is a single row with an ordered (requester, addressee) pair; the
-- "mutual flame" between two accepted friends is derived from shared check-in
-- nights (no stored counter), so it stays consistent with the check_in table.

CREATE TABLE friendship (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    requester_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    addressee_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status       TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (requester_id <> addressee_id),
    UNIQUE (requester_id, addressee_id)
);

-- A pair may only exist once regardless of who asked first.
CREATE UNIQUE INDEX friendship_pair_idx
    ON friendship (LEAST(requester_id, addressee_id), GREATEST(requester_id, addressee_id));

CREATE INDEX friendship_addressee_idx ON friendship (addressee_id, status);
CREATE INDEX friendship_requester_idx ON friendship (requester_id, status);

CREATE TRIGGER trg_friendship_updated_at
    BEFORE UPDATE ON friendship
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();
