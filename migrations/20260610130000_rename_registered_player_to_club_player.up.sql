-- Rename the club roster identity from `registered_player` to `club_player`.
--
-- "registered_player" was ambiguous next to `tournament_registrations` (where
-- "registered" means signed up for an event). `club_player` is neutral: the
-- club-scoped player identity, whether or not they have an app account.
--
-- Renames the table, the `registered_player_id` FK on the four operational
-- tables, the related indexes/trigger, and redefines the three trigger
-- functions whose bodies reference the renamed table/column (PL/pgSQL bodies
-- are not auto-updated by RENAME).

-- 1. Drop the link triggers + function (rebuilt below under the new name).
DROP TRIGGER IF EXISTS trg_link_registered_player_registrations ON tournament_registrations;
DROP TRIGGER IF EXISTS trg_link_registered_player_results ON tournament_results;
DROP TRIGGER IF EXISTS trg_link_registered_player_entries ON tournament_entries;
DROP TRIGGER IF EXISTS trg_link_registered_player_seats ON table_seat_assignments;
DROP FUNCTION IF EXISTS link_registered_player();

-- 2. Rename the table and the FK columns.
ALTER TABLE registered_player RENAME TO club_player;
ALTER TABLE tournament_registrations RENAME COLUMN registered_player_id TO club_player_id;
ALTER TABLE tournament_results RENAME COLUMN registered_player_id TO club_player_id;
ALTER TABLE tournament_entries RENAME COLUMN registered_player_id TO club_player_id;
ALTER TABLE table_seat_assignments RENAME COLUMN registered_player_id TO club_player_id;

-- 3. Rename indexes and the updated_at trigger that embed the old name.
ALTER INDEX registered_player_club_id_idx RENAME TO club_player_club_id_idx;
ALTER INDEX registered_player_app_user_id_idx RENAME TO club_player_app_user_id_idx;
ALTER INDEX registered_player_club_app_user_uniq RENAME TO club_player_club_app_user_uniq;
ALTER INDEX registered_player_is_active_idx RENAME TO club_player_is_active_idx;
ALTER INDEX tournament_registrations_registered_player_id_idx RENAME TO tournament_registrations_club_player_id_idx;
ALTER INDEX tournament_results_registered_player_id_idx RENAME TO tournament_results_club_player_id_idx;
ALTER INDEX tournament_entries_registered_player_id_idx RENAME TO tournament_entries_club_player_id_idx;
ALTER INDEX table_seat_assignments_registered_player_id_idx RENAME TO table_seat_assignments_club_player_id_idx;
ALTER TRIGGER trg_registered_player_updated_at ON club_player RENAME TO trg_club_player_updated_at;

-- 4. Recreate the roster-link trigger function under the new name.
CREATE OR REPLACE FUNCTION link_club_player()
RETURNS TRIGGER AS $$
DECLARE
    v_club_id UUID;
    v_cp_id   UUID;
    v_name    TEXT;
BEGIN
    SELECT club_id INTO v_club_id FROM tournaments WHERE id = NEW.tournament_id;
    IF v_club_id IS NULL THEN
        RETURN NEW;
    END IF;

    IF NEW.club_player_id IS NOT NULL THEN
        IF NEW.user_id IS NULL THEN
            SELECT app_user_id INTO NEW.user_id
                FROM club_player WHERE id = NEW.club_player_id;
        END IF;
        RETURN NEW;
    END IF;

    IF NEW.user_id IS NULL THEN
        RETURN NEW;
    END IF;

    SELECT id INTO v_cp_id FROM club_player
        WHERE club_id = v_club_id AND app_user_id = NEW.user_id;

    IF v_cp_id IS NULL THEN
        SELECT COALESCE(
            NULLIF(TRIM(COALESCE(first_name, '') || ' ' || COALESCE(last_name, '')), ''),
            username, email, 'Unknown'
        ) INTO v_name FROM users WHERE id = NEW.user_id;

        INSERT INTO club_player (club_id, display_name, app_user_id)
            VALUES (v_club_id, COALESCE(v_name, 'Unknown'), NEW.user_id)
            RETURNING id INTO v_cp_id;
    END IF;

    NEW.club_player_id := v_cp_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_link_club_player_registrations
    BEFORE INSERT ON tournament_registrations
    FOR EACH ROW EXECUTE PROCEDURE link_club_player();
CREATE TRIGGER trg_link_club_player_results
    BEFORE INSERT ON tournament_results
    FOR EACH ROW EXECUTE PROCEDURE link_club_player();
CREATE TRIGGER trg_link_club_player_entries
    BEFORE INSERT ON tournament_entries
    FOR EACH ROW EXECUTE PROCEDURE link_club_player();
CREATE TRIGGER trg_link_club_player_seats
    BEFORE INSERT ON table_seat_assignments
    FOR EACH ROW EXECUTE PROCEDURE link_club_player();

-- 5. Redefine the prize-pool function to count by the renamed column.
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

    SELECT
        COALESCE(SUM(amount_cents) FILTER (WHERE entry_type NOT IN ('voucher', 'bonus')), 0),
        COUNT(DISTINCT club_player_id) FILTER (WHERE entry_type NOT IN ('voucher', 'bonus'))
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

-- 6. Redefine the seat-reassignment function to key on the renamed column.
CREATE OR REPLACE FUNCTION handle_seat_assignment()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.is_current = true THEN
        UPDATE table_seat_assignments
        SET is_current = false,
            unassigned_at = NOW(),
            updated_at = NOW()
        WHERE tournament_id = NEW.tournament_id
          AND club_player_id = NEW.club_player_id
          AND is_current = true
          AND id != NEW.id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
