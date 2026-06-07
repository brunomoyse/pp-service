-- Roster identity flip.
--
-- `registered_player` (the club roster) becomes the canonical player identity on
-- the four operational tables. `user_id` becomes OPTIONAL — it is populated only
-- when the player has an onboarded app account. Account-less players (no app
-- user) are operated entirely by the club via the manager app; they can be
-- registered, bought in, charged the voucher, seated, and scored, and they
-- appear in results and on the leaderboard by their roster `display_name`.

-- 1. Add the roster FK to entries and seat assignments (registrations/results
--    already gained it in 20260606120000).
ALTER TABLE tournament_entries
    ADD COLUMN registered_player_id UUID REFERENCES registered_player(id) ON DELETE SET NULL;
ALTER TABLE table_seat_assignments
    ADD COLUMN registered_player_id UUID REFERENCES registered_player(id) ON DELETE SET NULL;

-- 2. Ensure a roster entry exists for everyone already in entries/seating, then
--    backfill registered_player_id across all four tables.
INSERT INTO registered_player (club_id, display_name, app_user_id)
SELECT DISTINCT
    t.club_id,
    COALESCE(
        NULLIF(TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, '')), ''),
        u.username, u.email, 'Unknown'
    ),
    u.id
FROM (
    SELECT tournament_id, user_id FROM tournament_entries
    UNION
    SELECT tournament_id, user_id FROM table_seat_assignments
) src
JOIN tournaments t ON t.id = src.tournament_id
JOIN users u ON u.id = src.user_id
ON CONFLICT DO NOTHING;

UPDATE tournament_registrations r SET registered_player_id = rp.id
FROM tournaments t, registered_player rp
WHERE r.tournament_id = t.id AND rp.club_id = t.club_id AND rp.app_user_id = r.user_id
  AND r.registered_player_id IS NULL;
UPDATE tournament_results r SET registered_player_id = rp.id
FROM tournaments t, registered_player rp
WHERE r.tournament_id = t.id AND rp.club_id = t.club_id AND rp.app_user_id = r.user_id
  AND r.registered_player_id IS NULL;
UPDATE tournament_entries e SET registered_player_id = rp.id
FROM tournaments t, registered_player rp
WHERE e.tournament_id = t.id AND rp.club_id = t.club_id AND rp.app_user_id = e.user_id
  AND e.registered_player_id IS NULL;
UPDATE table_seat_assignments s SET registered_player_id = rp.id
FROM tournaments t, registered_player rp
WHERE s.tournament_id = t.id AND rp.club_id = t.club_id AND rp.app_user_id = s.user_id
  AND s.registered_player_id IS NULL;

-- 3. Flip nullability: roster id required, user id optional.
ALTER TABLE tournament_registrations
    ALTER COLUMN registered_player_id SET NOT NULL,
    ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE tournament_results
    ALTER COLUMN registered_player_id SET NOT NULL,
    ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE tournament_entries
    ALTER COLUMN registered_player_id SET NOT NULL,
    ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE table_seat_assignments
    ALTER COLUMN registered_player_id SET NOT NULL,
    ALTER COLUMN user_id DROP NOT NULL;

-- 4. Move uniqueness from user_id to registered_player_id.
ALTER TABLE tournament_registrations
    DROP CONSTRAINT tournament_registrations_tournament_id_user_id_key,
    ADD CONSTRAINT tournament_registrations_tournament_id_rp_key UNIQUE (tournament_id, registered_player_id);
ALTER TABLE tournament_results
    DROP CONSTRAINT tournament_results_tournament_id_user_id_key,
    ADD CONSTRAINT tournament_results_tournament_id_rp_key UNIQUE (tournament_id, registered_player_id);

DROP INDEX table_seat_assignments_unique_current_player;
CREATE UNIQUE INDEX table_seat_assignments_unique_current_player
    ON table_seat_assignments (tournament_id, registered_player_id) WHERE is_current = true;

CREATE INDEX tournament_entries_registered_player_id_idx ON tournament_entries (registered_player_id);
CREATE INDEX table_seat_assignments_registered_player_id_idx ON table_seat_assignments (registered_player_id);

-- 5. Link trigger: roster is canonical. When a row arrives with only
--    registered_player_id, backfill user_id from the roster (NULL for
--    account-less players). When a row arrives with only user_id (app-user
--    self-service path), find-or-create the roster entry as before. Now also
--    fires on entries and seat assignments.
CREATE OR REPLACE FUNCTION link_registered_player()
RETURNS TRIGGER AS $$
DECLARE
    v_club_id UUID;
    v_rp_id   UUID;
    v_name    TEXT;
BEGIN
    SELECT club_id INTO v_club_id FROM tournaments WHERE id = NEW.tournament_id;
    IF v_club_id IS NULL THEN
        RETURN NEW;
    END IF;

    IF NEW.registered_player_id IS NOT NULL THEN
        IF NEW.user_id IS NULL THEN
            SELECT app_user_id INTO NEW.user_id
                FROM registered_player WHERE id = NEW.registered_player_id;
        END IF;
        RETURN NEW;
    END IF;

    -- No roster id supplied: need a user to find-or-create from.
    IF NEW.user_id IS NULL THEN
        RETURN NEW;
    END IF;

    SELECT id INTO v_rp_id FROM registered_player
        WHERE club_id = v_club_id AND app_user_id = NEW.user_id;

    IF v_rp_id IS NULL THEN
        SELECT COALESCE(
            NULLIF(TRIM(COALESCE(first_name, '') || ' ' || COALESCE(last_name, '')), ''),
            username, email, 'Unknown'
        ) INTO v_name FROM users WHERE id = NEW.user_id;

        INSERT INTO registered_player (club_id, display_name, app_user_id)
            VALUES (v_club_id, COALESCE(v_name, 'Unknown'), NEW.user_id)
            RETURNING id INTO v_rp_id;
    END IF;

    NEW.registered_player_id := v_rp_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_link_registered_player_entries
    BEFORE INSERT ON tournament_entries
    FOR EACH ROW EXECUTE PROCEDURE link_registered_player();
CREATE TRIGGER trg_link_registered_player_seats
    BEFORE INSERT ON table_seat_assignments
    FOR EACH ROW EXECUTE PROCEDURE link_registered_player();

-- 6. Prize-pool player count must key on the roster (account-less players carry
--    NULL user_id). Body otherwise unchanged from 20260217130000 (UPSERT).
CREATE OR REPLACE FUNCTION recalculate_prize_pool_from_entries()
RETURNS TRIGGER AS $$
DECLARE
    v_tournament_id UUID;
    v_total_amount INTEGER;
    v_player_count INTEGER;
    v_template RECORD;
    v_payout_positions JSONB;
BEGIN
    v_tournament_id := COALESCE(NEW.tournament_id, OLD.tournament_id);

    SELECT COALESCE(SUM(amount_cents), 0), COUNT(DISTINCT registered_player_id)
    INTO v_total_amount, v_player_count
    FROM tournament_entries WHERE tournament_id = v_tournament_id;

    SELECT * INTO v_template FROM payout_templates
    WHERE min_players <= v_player_count
    AND (max_players IS NULL OR max_players >= v_player_count)
    ORDER BY min_players DESC LIMIT 1;

    IF v_template.id IS NOT NULL THEN
        SELECT to_jsonb(array_agg(
            jsonb_build_object(
                'position', (pos->>'position')::INTEGER,
                'amount_cents', FLOOR((pos->>'percentage')::NUMERIC * v_total_amount / 100),
                'percentage', (pos->>'percentage')::NUMERIC
            ) ORDER BY (pos->>'position')::INTEGER
        )) INTO v_payout_positions
        FROM jsonb_array_elements(v_template.payout_structure) pos;
    ELSE
        v_payout_positions := '[]'::JSONB;
    END IF;

    INSERT INTO tournament_payouts (
        tournament_id, template_id, player_count, total_prize_pool, payout_positions
    ) VALUES (
        v_tournament_id, v_template.id, v_player_count, v_total_amount,
        COALESCE(v_payout_positions, '[]'::JSONB)
    )
    ON CONFLICT (tournament_id) DO UPDATE SET
        total_prize_pool = EXCLUDED.total_prize_pool,
        player_count = EXCLUDED.player_count,
        template_id = EXCLUDED.template_id,
        payout_positions = EXCLUDED.payout_positions,
        updated_at = NOW();

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

-- 7. Seat reassignment must key on the roster too.
CREATE OR REPLACE FUNCTION handle_seat_assignment()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.is_current = true THEN
        UPDATE table_seat_assignments
        SET is_current = false,
            unassigned_at = NOW(),
            updated_at = NOW()
        WHERE tournament_id = NEW.tournament_id
          AND registered_player_id = NEW.registered_player_id
          AND is_current = true
          AND id != NEW.id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
