-- Create achievements catalog table
CREATE TABLE achievements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(100) UNIQUE NOT NULL,
    name_key VARCHAR(150) NOT NULL,
    description_key VARCHAR(200) NOT NULL,
    category VARCHAR(30) NOT NULL,
    icon VARCHAR(60),
    tier VARCHAR(20),
    threshold_value INT,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER trg_achievements_updated_at BEFORE UPDATE ON achievements
FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- Create player achievements progress/unlock tracking
CREATE TABLE player_achievements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    achievement_id UUID NOT NULL REFERENCES achievements(id) ON DELETE CASCADE,
    unlocked_at TIMESTAMPTZ,
    progress INT NOT NULL DEFAULT 0,
    tournament_id UUID REFERENCES tournaments(id) ON DELETE SET NULL,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, achievement_id)
);

CREATE INDEX idx_player_achievements_user_id ON player_achievements(user_id);
CREATE INDEX idx_player_achievements_unlocked_at ON player_achievements(unlocked_at DESC);

CREATE TRIGGER trg_player_achievements_updated_at BEFORE UPDATE ON player_achievements
FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- Seed the achievement catalog
INSERT INTO achievements (code, name_key, description_key, category, icon, tier, threshold_value)
VALUES
    ('first_registration', 'achievements.items.first_registration.name', 'achievements.items.first_registration.description', 'registration', 'ticket-outline', 'bronze', NULL),
    ('first_cash', 'achievements.items.first_cash.name', 'achievements.items.first_cash.description', 'winnings', 'cash-outline', 'bronze', NULL),
    ('first_win', 'achievements.items.first_win.name', 'achievements.items.first_win.description', 'winnings', 'trophy-outline', 'gold', NULL),
    ('tournaments_5', 'achievements.items.tournaments_5.name', 'achievements.items.tournaments_5.description', 'milestones', 'flash-outline', 'bronze', 5),
    ('tournaments_20', 'achievements.items.tournaments_20.name', 'achievements.items.tournaments_20.description', 'milestones', 'flame-outline', 'silver', 20),
    ('tournaments_50', 'achievements.items.tournaments_50.name', 'achievements.items.tournaments_50.description', 'milestones', 'ribbon-outline', 'gold', 50),
    ('final_table_5', 'achievements.items.final_table_5.name', 'achievements.items.final_table_5.description', 'results', 'people-outline', 'silver', 5),
    ('winnings_1000', 'achievements.items.winnings_1000.name', 'achievements.items.winnings_1000.description', 'milestones', 'diamond-outline', 'silver', 100000),
    ('itm_rate_50', 'achievements.items.itm_rate_50.name', 'achievements.items.itm_rate_50.description', 'results', 'stats-chart-outline', 'silver', 50),
    ('rebuy_3', 'achievements.items.rebuy_3.name', 'achievements.items.rebuy_3.description', 'results', 'refresh-outline', 'bronze', 3),
    ('streak_cash_3', 'achievements.items.streak_cash_3.name', 'achievements.items.streak_cash_3.description', 'streaks', 'trending-up-outline', 'silver', 3),
    ('streak_play_5', 'achievements.items.streak_play_5.name', 'achievements.items.streak_play_5.description', 'streaks', 'bonfire-outline', 'gold', 5)
ON CONFLICT (code) DO NOTHING;
