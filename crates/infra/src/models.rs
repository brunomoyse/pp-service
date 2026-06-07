use crate::repos::tournaments::TournamentLiveStatus;
use chrono::{DateTime, NaiveDate, Utc};
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
    pub rake_cents: i32,
    pub seat_cap: Option<i32>,
    pub live_status: TournamentLiveStatus,
    pub early_bird_bonus_chips: Option<i32>,
    pub late_registration_level: Option<i32>,
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
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub username: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
    pub role: Option<String>,
    pub locale: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TournamentRegistrationRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub user_id: Option<Uuid>,
    pub registered_player_id: Uuid,
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
    pub user_id: Option<Uuid>,
    pub registered_player_id: Uuid,
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
    pub max_seats_override: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TableSeatAssignmentRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub club_table_id: Uuid,
    pub user_id: Option<Uuid>,
    pub registered_player_id: Uuid,
    pub seat_number: i32,
    pub stack_size: Option<i32>,
    pub is_current: bool,
    pub assigned_at: DateTime<Utc>,
    pub unassigned_at: Option<DateTime<Utc>>,
    pub assigned_by: Option<Uuid>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
pub struct TournamentActivityLogRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub event_category: String,
    pub event_action: String,
    pub actor_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
    pub event_time: DateTime<Utc>,
    pub metadata: serde_json::Value,
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
pub struct CheckInRow {
    pub id: Uuid,
    pub app_user_id: Uuid,
    pub tournament_id: Uuid,
    pub club_id: Uuid,
    pub checked_in_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct AttendanceStreakRow {
    pub app_user_id: Uuid,
    pub current_streak: i32,
    pub longest_streak: i32,
    pub last_check_in_at: Option<DateTime<Utc>>,
    pub freezes_available: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct SeasonRow {
    pub id: Uuid,
    pub club_id: Uuid,
    pub name: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct SeasonPassRow {
    pub id: Uuid,
    pub season_id: Uuid,
    pub app_user_id: Uuid,
    pub is_premium: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct QuestCompletionRow {
    pub id: Uuid,
    pub app_user_id: Uuid,
    pub quest_code: String,
    pub week_start: NaiveDate,
    pub xp_awarded: i32,
    pub completed_at: DateTime<Utc>,
}

/// Aggregate result: a season's most-present player (Hall of Fame champion).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct SeasonChampionRow {
    pub app_user_id: Uuid,
    pub champion_name: String,
    pub events: i64,
}

/// Aggregate result: head-to-head record against one opponent.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct RivalryRow {
    pub opponent_id: Uuid,
    pub opponent_name: String,
    pub meetings: i64,
    /// Tournaments where the subject finished above this opponent.
    pub wins: i64,
    /// Tournaments where this opponent finished above the subject.
    pub losses: i64,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct FriendshipRow {
    pub id: Uuid,
    pub requester_id: Uuid,
    pub addressee_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A friendship resolved from the current user's perspective: the *other*
/// party plus the request direction.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct FriendRow {
    pub friendship_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub status: String,
    /// True when this is a pending request the current user received.
    pub is_incoming: bool,
}

/// Derived mutual-flame standing between two players.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct FlameRow {
    /// Distinct calendar nights both players checked in.
    pub shared_nights: i64,
    pub last_shared: Option<NaiveDate>,
}

/// Aggregate result: a player's tournament numbers for one calendar year.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct WrappedStatsRow {
    pub tournaments: i64,
    pub buyins_cents: i64,
    pub winnings_cents: i64,
    pub itm_count: i64,
    pub best_finish: Option<i32>,
}

/// Aggregate result: the club a player frequented most in a year.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct FavoriteClubRow {
    pub club_name: String,
    pub tournaments: i64,
}

// ---- Euro cosmetics economy (G1/G2) ----

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CosmeticItemRow {
    pub id: Uuid,
    pub code: String,
    pub kind: String,
    pub name: String,
    pub description: Option<String>,
    pub price_cents: i32,
    pub preview_ref: String,
    pub club_id: Option<Uuid>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct UserCosmeticRow {
    pub id: Uuid,
    pub app_user_id: Uuid,
    pub cosmetic_item_id: Uuid,
    pub source: String,
    pub equipped: bool,
    pub acquired_at: DateTime<Utc>,
}

// ---- Prediction Points economy (earned-only, G2) ----

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PredictionEntryRow {
    pub id: Uuid,
    pub app_user_id: Uuid,
    pub tournament_id: Uuid,
    pub predicted_winner_user_id: Uuid,
    pub stake_points: i32,
    pub status: String,
    pub payout_points: i32,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

// ---- Privacy / consent + scouting (G3/G4/G5) ----

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct UserPrivacySettingsRow {
    pub app_user_id: Uuid,
    pub share_named_pl: bool,
    pub in_scouting_pool: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A pool member matching a search (handle only — no stats until looked up).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ScoutingMatchRow {
    pub user_id: Uuid,
    pub handle: String,
}

/// A scouting profile's performance aggregates (lifetime).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ScoutingStatsRow {
    pub tournaments: i64,
    pub itm_count: i64,
    pub best_finish: Option<i32>,
    pub net_cents: i64,
}

/// A prediction enriched with display names for the client.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PredictionEntryView {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub tournament_name: String,
    pub predicted_winner_name: String,
    pub stake_points: i32,
    pub status: String,
    pub payout_points: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProEntitlementRow {
    pub id: Uuid,
    pub app_user_id: Uuid,
    pub source: String,
    pub granted_by_club_id: Option<Uuid>,
    pub granted_by_user_id: Option<Uuid>,
    pub starts_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PlayerNoteRow {
    pub id: Uuid,
    pub author_app_user_id: Uuid,
    pub subject_registered_player_id: Uuid,
    pub body: String,
    pub style: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PlayerNoteTagRow {
    pub id: Uuid,
    pub note_id: Uuid,
    pub kind: String,
    pub tag: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ShowdownObservationRow {
    pub id: Uuid,
    pub note_id: Uuid,
    pub tournament_id: Option<Uuid>,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct RegisteredPlayerRow {
    pub id: Uuid,
    pub club_id: Uuid,
    pub display_name: String,
    pub app_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct AchievementRow {
    pub id: Uuid,
    pub code: String,
    pub name_key: String,
    pub description_key: String,
    pub category: String,
    pub icon: Option<String>,
    pub tier: Option<String>,
    pub threshold_value: Option<i32>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PlayerAchievementRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub achievement_id: Uuid,
    pub unlocked_at: Option<DateTime<Utc>>,
    pub progress: i32,
    pub tournament_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
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

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TournamentEntryRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub user_id: Option<Uuid>,
    pub registered_player_id: Uuid,
    pub entry_type: String,
    pub amount_cents: i32,
    pub chips_received: Option<i32>,
    pub recorded_by: Option<Uuid>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct BlindStructureTemplateRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub levels: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
