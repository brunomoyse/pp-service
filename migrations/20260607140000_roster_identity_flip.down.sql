-- Revert the roster identity flip. Best-effort: requires that no rows have a
-- NULL user_id (i.e. no account-less players were created while this was live).

DROP TRIGGER IF EXISTS trg_link_registered_player_entries ON tournament_entries;
DROP TRIGGER IF EXISTS trg_link_registered_player_seats ON table_seat_assignments;

-- Restore seat reassignment keyed on user_id.
CREATE OR REPLACE FUNCTION handle_seat_assignment()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.is_current = true THEN
        UPDATE table_seat_assignments
        SET is_current = false, unassigned_at = NOW(), updated_at = NOW()
        WHERE tournament_id = NEW.tournament_id
          AND user_id = NEW.user_id
          AND is_current = true
          AND id != NEW.id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Restore prize-pool count keyed on user_id.
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
    SELECT COALESCE(SUM(amount_cents), 0), COUNT(DISTINCT user_id)
    INTO v_total_amount, v_player_count
    FROM tournament_entries WHERE tournament_id = v_tournament_id;
    SELECT * INTO v_template FROM payout_templates
    WHERE min_players <= v_player_count AND (max_players IS NULL OR max_players >= v_player_count)
    ORDER BY min_players DESC LIMIT 1;
    IF v_template.id IS NOT NULL THEN
        SELECT to_jsonb(array_agg(
            jsonb_build_object('position', (pos->>'position')::INTEGER,
                'amount_cents', FLOOR((pos->>'percentage')::NUMERIC * v_total_amount / 100),
                'percentage', (pos->>'percentage')::NUMERIC) ORDER BY (pos->>'position')::INTEGER
        )) INTO v_payout_positions FROM jsonb_array_elements(v_template.payout_structure) pos;
    ELSE
        v_payout_positions := '[]'::JSONB;
    END IF;
    INSERT INTO tournament_payouts (tournament_id, template_id, player_count, total_prize_pool, payout_positions)
    VALUES (v_tournament_id, v_template.id, v_player_count, v_total_amount, COALESCE(v_payout_positions, '[]'::JSONB))
    ON CONFLICT (tournament_id) DO UPDATE SET
        total_prize_pool = EXCLUDED.total_prize_pool, player_count = EXCLUDED.player_count,
        template_id = EXCLUDED.template_id, payout_positions = EXCLUDED.payout_positions, updated_at = NOW();
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

-- Restore link trigger to its user_id->roster-only form.
CREATE OR REPLACE FUNCTION link_registered_player()
RETURNS TRIGGER AS $$
DECLARE
    v_club_id UUID;
    v_rp_id   UUID;
    v_name    TEXT;
BEGIN
    IF NEW.registered_player_id IS NOT NULL THEN
        RETURN NEW;
    END IF;
    SELECT club_id INTO v_club_id FROM tournaments WHERE id = NEW.tournament_id;
    IF v_club_id IS NULL THEN
        RETURN NEW;
    END IF;
    SELECT id INTO v_rp_id FROM registered_player WHERE club_id = v_club_id AND app_user_id = NEW.user_id;
    IF v_rp_id IS NULL THEN
        SELECT COALESCE(NULLIF(TRIM(COALESCE(first_name, '') || ' ' || COALESCE(last_name, '')), ''),
            username, email, 'Unknown') INTO v_name FROM users WHERE id = NEW.user_id;
        INSERT INTO registered_player (club_id, display_name, app_user_id)
            VALUES (v_club_id, COALESCE(v_name, 'Unknown'), NEW.user_id) RETURNING id INTO v_rp_id;
    END IF;
    NEW.registered_player_id := v_rp_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Restore unique constraints/indexes on user_id.
DROP INDEX IF EXISTS table_seat_assignments_unique_current_player;
CREATE UNIQUE INDEX table_seat_assignments_unique_current_player
    ON table_seat_assignments (tournament_id, user_id) WHERE is_current = true;
ALTER TABLE tournament_results
    DROP CONSTRAINT IF EXISTS tournament_results_tournament_id_rp_key,
    ADD CONSTRAINT tournament_results_tournament_id_user_id_key UNIQUE (tournament_id, user_id);
ALTER TABLE tournament_registrations
    DROP CONSTRAINT IF EXISTS tournament_registrations_tournament_id_rp_key,
    ADD CONSTRAINT tournament_registrations_tournament_id_user_id_key UNIQUE (tournament_id, user_id);

-- Restore NOT NULL on user_id, drop NOT NULL on roster id.
ALTER TABLE table_seat_assignments ALTER COLUMN user_id SET NOT NULL, ALTER COLUMN registered_player_id DROP NOT NULL;
ALTER TABLE tournament_entries ALTER COLUMN user_id SET NOT NULL, ALTER COLUMN registered_player_id DROP NOT NULL;
ALTER TABLE tournament_results ALTER COLUMN user_id SET NOT NULL, ALTER COLUMN registered_player_id DROP NOT NULL;
ALTER TABLE tournament_registrations ALTER COLUMN user_id SET NOT NULL, ALTER COLUMN registered_player_id DROP NOT NULL;

DROP INDEX IF EXISTS tournament_entries_registered_player_id_idx;
DROP INDEX IF EXISTS table_seat_assignments_registered_player_id_idx;
ALTER TABLE tournament_entries DROP COLUMN registered_player_id;
ALTER TABLE table_seat_assignments DROP COLUMN registered_player_id;
