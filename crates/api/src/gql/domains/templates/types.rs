use async_graphql::{SimpleObject, ID};
use chrono::{DateTime, Utc};

/// A blind structure level without tournament_id (for templates)
#[derive(SimpleObject, Clone, Debug, serde::Deserialize)]
pub struct BlindStructureLevel {
    #[serde(rename = "levelNumber")]
    pub level_number: i32,
    #[serde(rename = "smallBlind")]
    pub small_blind: i32,
    #[serde(rename = "bigBlind")]
    pub big_blind: i32,
    pub ante: i32,
    #[serde(rename = "durationMinutes")]
    pub duration_minutes: i32,
    #[serde(rename = "isBreak")]
    pub is_break: bool,
    #[serde(rename = "breakDurationMinutes")]
    pub break_duration_minutes: Option<i32>,
}

#[derive(SimpleObject, Clone)]
pub struct BlindStructureTemplate {
    pub id: ID,
    pub name: String,
    pub description: Option<String>,
    pub levels: Vec<BlindStructureLevel>,
    pub created_at: DateTime<Utc>,
}
