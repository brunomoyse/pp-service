-- Reverse the club_player rename back to registered_player.

DROP TRIGGER IF EXISTS trg_link_club_player_registrations ON tournament_registrations;
DROP TRIGGER IF EXISTS trg_link_club_player_results ON tournament_results;
DROP TRIGGER IF EXISTS trg_link_club_player_entries ON tournament_entries;
DROP TRIGGER IF EXISTS trg_link_club_player_seats ON table_seat_assignments;
DROP FUNCTION IF EXISTS link_club_player();

ALTER TABLE club_player RENAME TO registered_player;
ALTER TABLE tournament_registrations RENAME COLUMN club_player_id TO registered_player_id;
ALTER TABLE tournament_results RENAME COLUMN club_player_id TO registered_player_id;
ALTER TABLE tournament_entries RENAME COLUMN club_player_id TO registered_player_id;
ALTER TABLE table_seat_assignments RENAME COLUMN club_player_id TO registered_player_id;

ALTER INDEX club_player_club_id_idx RENAME TO registered_player_club_id_idx;
ALTER INDEX club_player_app_user_id_idx RENAME TO registered_player_app_user_id_idx;
ALTER INDEX club_player_club_app_user_uniq RENAME TO registered_player_club_app_user_uniq;
ALTER INDEX club_player_is_active_idx RENAME TO registered_player_is_active_idx;
ALTER INDEX tournament_registrations_club_player_id_idx RENAME TO tournament_registrations_registered_player_id_idx;
ALTER INDEX tournament_results_club_player_id_idx RENAME TO tournament_results_registered_player_id_idx;
ALTER INDEX tournament_entries_club_player_id_idx RENAME TO tournament_entries_registered_player_id_idx;
ALTER INDEX table_seat_assignments_club_player_id_idx RENAME TO table_seat_assignments_registered_player_id_idx;
ALTER TRIGGER trg_club_player_updated_at ON registered_player RENAME TO trg_registered_player_updated_at;

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

CREATE TRIGGER trg_link_registered_player_registrations
    BEFORE INSERT ON tournament_registrations
    FOR EACH ROW EXECUTE PROCEDURE link_registered_player();
CREATE TRIGGER trg_link_registered_player_results
    BEFORE INSERT ON tournament_results
    FOR EACH ROW EXECUTE PROCEDURE link_registered_player();
CREATE TRIGGER trg_link_registered_player_entries
    BEFORE INSERT ON tournament_entries
    FOR EACH ROW EXECUTE PROCEDURE link_registered_player();
CREATE TRIGGER trg_link_registered_player_seats
    BEFORE INSERT ON table_seat_assignments
    FOR EACH ROW EXECUTE PROCEDURE link_registered_player();

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
        COUNT(DISTINCT registered_player_id) FILTER (WHERE entry_type NOT IN ('voucher', 'bonus'))
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
