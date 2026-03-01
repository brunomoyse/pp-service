use async_graphql::{InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};

// ── Output types ──

/// A payout structure entry within a payout template
#[derive(SimpleObject, Clone, Debug, serde::Deserialize)]
pub struct PayoutStructureEntry {
    pub position: i32,
    pub percentage: f64,
}

/// A reusable payout template defining prize distribution percentages
#[derive(SimpleObject, Clone)]
pub struct PayoutTemplate {
    pub id: ID,
    pub name: String,
    pub description: Option<String>,
    pub min_players: i32,
    pub max_players: Option<i32>,
    pub payout_structure: Vec<PayoutStructureEntry>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

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
    pub updated_at: DateTime<Utc>,
}

// ── Input types ──

#[derive(InputObject)]
pub struct PayoutStructureEntryInput {
    pub position: i32,
    pub percentage: f64,
}

#[derive(InputObject)]
pub struct CreatePayoutTemplateInput {
    pub name: String,
    pub description: Option<String>,
    pub min_players: i32,
    pub max_players: Option<i32>,
    pub payout_structure: Vec<PayoutStructureEntryInput>,
}

#[derive(InputObject)]
pub struct UpdatePayoutTemplateInput {
    pub id: ID,
    pub name: String,
    pub description: Option<String>,
    pub min_players: i32,
    pub max_players: Option<i32>,
    pub payout_structure: Vec<PayoutStructureEntryInput>,
}

#[derive(InputObject)]
pub struct BlindStructureLevelInput {
    pub level_number: i32,
    pub small_blind: i32,
    pub big_blind: i32,
    pub ante: i32,
    pub duration_minutes: i32,
    pub is_break: bool,
    pub break_duration_minutes: Option<i32>,
}

#[derive(InputObject)]
pub struct CreateBlindStructureTemplateInput {
    pub name: String,
    pub description: Option<String>,
    pub levels: Vec<BlindStructureLevelInput>,
}

#[derive(InputObject)]
pub struct UpdateBlindStructureTemplateInput {
    pub id: ID,
    pub name: String,
    pub description: Option<String>,
    pub levels: Vec<BlindStructureLevelInput>,
}
