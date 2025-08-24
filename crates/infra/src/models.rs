use crate::repos::tournaments::TournamentLiveStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ClubRow {
    pub id: Uuid,
    pub name: String,
    pub city: Option<String>,
    pub country: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TournamentRow {
    pub id: Uuid,
    pub club_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub buy_in_cents: i32,
    pub seat_cap: Option<i32>,
    pub live_status: TournamentLiveStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TournamentRow {
    /// Calculate static status based on live status
    pub fn calculate_status(&self) -> crate::repos::tournaments::TournamentStatus {
        use crate::repos::tournaments::TournamentLiveStatus as LiveStatus;
        use crate::repos::tournaments::TournamentStatus;

        match self.live_status {
            LiveStatus::NotStarted | LiveStatus::RegistrationOpen => TournamentStatus::Upcoming,
            LiveStatus::LateRegistration
            | LiveStatus::InProgress
            | LiveStatus::Break
            | LiveStatus::FinalTable => TournamentStatus::InProgress,
            LiveStatus::Finished => TournamentStatus::Completed,
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TournamentStateRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub current_level: Option<i32>,
    pub players_remaining: Option<i32>,
    pub break_until: Option<DateTime<Utc>>,
    pub current_small_blind: Option<i32>,
    pub current_big_blind: Option<i32>,
    pub current_ante: Option<i32>,
    pub level_started_at: Option<DateTime<Utc>>,
    pub level_duration_minutes: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub username: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
    pub role: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TournamentRegistrationRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub user_id: Uuid,
    pub registration_time: DateTime<Utc>,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TournamentResultRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub user_id: Uuid,
    pub final_position: i32,
    pub prize_cents: i32,
    pub points: i32,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PayoutTemplateRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub min_players: i32,
    pub max_players: Option<i32>,
    pub payout_structure: serde_json::Value, // JSONB field
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PlayerDealRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub deal_type: String,
    pub affected_positions: Vec<i32>,
    pub custom_payouts: Option<serde_json::Value>, // JSONB field
    pub total_amount_cents: i32,
    pub notes: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TournamentTableRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub table_number: i32,
    pub max_seats: i32,
    pub is_active: bool,
    pub table_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ClubTableRow {
    pub id: Uuid,
    pub club_id: Uuid,
    pub table_number: i32,
    pub max_seats: i32,
    pub table_name: Option<String>,
    pub location: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TournamentTableAssignmentRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub club_table_id: Uuid,
    pub is_active: bool,
    pub assigned_at: DateTime<Utc>,
    pub deactivated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TableSeatAssignmentRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub club_table_id: Uuid,
    pub user_id: Uuid,
    pub seat_number: i32,
    pub stack_size: Option<i32>,
    pub is_current: bool,
    pub assigned_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TournamentStructureRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub level_number: i32,
    pub small_blind: i32,
    pub big_blind: i32,
    pub ante: i32,
    pub duration_minutes: i32,
    pub is_break: bool,
    pub break_duration_minutes: Option<i32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TournamentClockRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub clock_status: String,
    pub current_level: i32,
    pub level_started_at: Option<DateTime<Utc>>,
    pub level_end_time: Option<DateTime<Utc>>,
    pub pause_started_at: Option<DateTime<Utc>>,
    #[serde(skip)]
    pub total_pause_duration: sqlx::postgres::types::PgInterval,
    pub auto_advance: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TournamentClockEventRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub event_type: String,
    pub level_number: Option<i32>,
    pub manager_id: Option<Uuid>,
    pub event_time: DateTime<Utc>,
    pub metadata: serde_json::Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ClubManagerRow {
    pub id: Uuid,
    pub club_id: Uuid,
    pub user_id: Uuid,
    pub assigned_at: DateTime<Utc>,
    pub assigned_by: Option<Uuid>,
    pub is_active: bool,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TournamentPayoutRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub template_id: Option<Uuid>,
    pub player_count: i32,
    pub total_prize_pool: i32,
    pub payout_positions: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
