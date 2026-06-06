use async_graphql::{Enum, SimpleObject, ID};
use chrono::{DateTime, Utc};

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum AchievementCategory {
    Registration,
    Winnings,
    Results,
    Milestones,
    Streaks,
}

impl From<String> for AchievementCategory {
    fn from(s: String) -> Self {
        match s.as_str() {
            "registration" => AchievementCategory::Registration,
            "winnings" => AchievementCategory::Winnings,
            "results" => AchievementCategory::Results,
            "milestones" => AchievementCategory::Milestones,
            "streaks" => AchievementCategory::Streaks,
            _ => AchievementCategory::Milestones,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum AchievementTier {
    Bronze,
    Silver,
    Gold,
    Platinum,
    Legendary,
}

impl From<Option<String>> for AchievementTier {
    fn from(s: Option<String>) -> Self {
        match s.as_deref() {
            Some("bronze") => AchievementTier::Bronze,
            Some("silver") => AchievementTier::Silver,
            Some("gold") => AchievementTier::Gold,
            Some("platinum") => AchievementTier::Platinum,
            Some("legendary") => AchievementTier::Legendary,
            _ => AchievementTier::Bronze,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct Achievement {
    pub id: ID,
    pub code: String,
    pub name_key: String,
    pub description_key: String,
    pub category: AchievementCategory,
    pub icon: Option<String>,
    pub tier: Option<AchievementTier>,
    pub threshold_value: Option<i32>,
}

impl From<infra::models::AchievementRow> for Achievement {
    fn from(row: infra::models::AchievementRow) -> Self {
        Achievement {
            id: row.id.into(),
            code: row.code,
            name_key: row.name_key,
            description_key: row.description_key,
            category: AchievementCategory::from(row.category),
            icon: row.icon,
            tier: AchievementTier::from(row.tier).into(),
            threshold_value: row.threshold_value,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct PlayerAchievement {
    pub id: ID,
    pub achievement: Achievement,
    pub progress: i32,
    pub unlocked_at: Option<DateTime<Utc>>,
    pub is_locked: bool,
}

/// Helper to construct PlayerAchievement from (AchievementRow, Option<PlayerAchievementRow>)
impl
    From<(
        infra::models::AchievementRow,
        Option<infra::models::PlayerAchievementRow>,
    )> for PlayerAchievement
{
    fn from(
        (achievement_row, player_row): (
            infra::models::AchievementRow,
            Option<infra::models::PlayerAchievementRow>,
        ),
    ) -> Self {
        let unlocked_at = player_row.as_ref().and_then(|pa| pa.unlocked_at);
        let progress = player_row.as_ref().map(|pa| pa.progress).unwrap_or(0);
        let is_locked = unlocked_at.is_none();

        PlayerAchievement {
            id: player_row
                .as_ref()
                .map(|pa| pa.id)
                .unwrap_or(achievement_row.id)
                .into(),
            achievement: Achievement::from(achievement_row),
            progress,
            unlocked_at,
            is_locked,
        }
    }
}
