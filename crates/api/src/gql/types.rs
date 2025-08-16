use async_graphql::dataloader::DataLoader;
use async_graphql::{ComplexObject, Context, Enum, Error, InputObject, Result, SimpleObject, ID};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::gql::loaders::ClubLoader;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Role {
    #[graphql(name = "ADMIN")]
    Admin,
    #[graphql(name = "MANAGER")]
    Manager,
    #[graphql(name = "PLAYER")]
    Player,
}

impl From<String> for Role {
    fn from(role: String) -> Self {
        match role.as_str() {
            "admin" => Role::Admin,
            "manager" => Role::Manager,
            "player" => Role::Player,
            _ => Role::Player, // Default to player for invalid roles
        }
    }
}

impl From<Option<String>> for Role {
    fn from(role: Option<String>) -> Self {
        match role {
            Some(r) => Role::from(r),
            None => Role::Player, // Default to player if no role specified
        }
    }
}

impl From<Role> for String {
    fn from(role: Role) -> Self {
        match role {
            Role::Admin => "admin".to_string(),
            Role::Manager => "manager".to_string(),
            Role::Player => "player".to_string(),
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum TournamentStatus {
    #[graphql(name = "UPCOMING")]
    Upcoming,
    #[graphql(name = "IN_PROGRESS")]
    InProgress,
    #[graphql(name = "COMPLETED")]
    Completed,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum TournamentLiveStatus {
    #[graphql(name = "NOT_STARTED")]
    NotStarted,
    #[graphql(name = "REGISTRATION_OPEN")]
    RegistrationOpen,
    #[graphql(name = "LATE_REGISTRATION")]
    LateRegistration,
    #[graphql(name = "IN_PROGRESS")]
    InProgress,
    #[graphql(name = "BREAK")]
    Break,
    #[graphql(name = "FINAL_TABLE")]
    FinalTable,
    #[graphql(name = "FINISHED")]
    Finished,
}

impl From<TournamentStatus> for infra::repos::tournaments::TournamentStatus {
    fn from(status: TournamentStatus) -> Self {
        match status {
            TournamentStatus::Upcoming => infra::repos::tournaments::TournamentStatus::Upcoming,
            TournamentStatus::InProgress => infra::repos::tournaments::TournamentStatus::InProgress,
            TournamentStatus::Completed => infra::repos::tournaments::TournamentStatus::Completed,
        }
    }
}

impl From<infra::repos::tournaments::TournamentStatus> for TournamentStatus {
    fn from(status: infra::repos::tournaments::TournamentStatus) -> Self {
        match status {
            infra::repos::tournaments::TournamentStatus::Upcoming => TournamentStatus::Upcoming,
            infra::repos::tournaments::TournamentStatus::InProgress => TournamentStatus::InProgress,
            infra::repos::tournaments::TournamentStatus::Completed => TournamentStatus::Completed,
        }
    }
}

impl From<String> for TournamentLiveStatus {
    fn from(status: String) -> Self {
        match status.as_str() {
            "not_started" => TournamentLiveStatus::NotStarted,
            "registration_open" => TournamentLiveStatus::RegistrationOpen,
            "late_registration" => TournamentLiveStatus::LateRegistration,
            "in_progress" => TournamentLiveStatus::InProgress,
            "break" => TournamentLiveStatus::Break,
            "final_table" => TournamentLiveStatus::FinalTable,
            "finished" => TournamentLiveStatus::Finished,
            _ => TournamentLiveStatus::NotStarted, // Default to not_started for invalid statuses
        }
    }
}

impl From<Option<String>> for TournamentLiveStatus {
    fn from(status: Option<String>) -> Self {
        match status {
            Some(s) => TournamentLiveStatus::from(s),
            None => TournamentLiveStatus::NotStarted, // Default to not_started if no status specified
        }
    }
}

impl From<TournamentLiveStatus> for String {
    fn from(status: TournamentLiveStatus) -> Self {
        match status {
            TournamentLiveStatus::NotStarted => "not_started".to_string(),
            TournamentLiveStatus::RegistrationOpen => "registration_open".to_string(),
            TournamentLiveStatus::LateRegistration => "late_registration".to_string(),
            TournamentLiveStatus::InProgress => "in_progress".to_string(),
            TournamentLiveStatus::Break => "break".to_string(),
            TournamentLiveStatus::FinalTable => "final_table".to_string(),
            TournamentLiveStatus::Finished => "finished".to_string(),
        }
    }
}

impl From<infra::repos::tournaments::TournamentLiveStatus> for TournamentLiveStatus {
    fn from(status: infra::repos::tournaments::TournamentLiveStatus) -> Self {
        match status {
            infra::repos::tournaments::TournamentLiveStatus::NotStarted => {
                TournamentLiveStatus::NotStarted
            }
            infra::repos::tournaments::TournamentLiveStatus::RegistrationOpen => {
                TournamentLiveStatus::RegistrationOpen
            }
            infra::repos::tournaments::TournamentLiveStatus::LateRegistration => {
                TournamentLiveStatus::LateRegistration
            }
            infra::repos::tournaments::TournamentLiveStatus::InProgress => {
                TournamentLiveStatus::InProgress
            }
            infra::repos::tournaments::TournamentLiveStatus::Break => TournamentLiveStatus::Break,
            infra::repos::tournaments::TournamentLiveStatus::FinalTable => {
                TournamentLiveStatus::FinalTable
            }
            infra::repos::tournaments::TournamentLiveStatus::Finished => {
                TournamentLiveStatus::Finished
            }
        }
    }
}

impl From<TournamentLiveStatus> for infra::repos::tournaments::TournamentLiveStatus {
    fn from(status: TournamentLiveStatus) -> Self {
        match status {
            TournamentLiveStatus::NotStarted => {
                infra::repos::tournaments::TournamentLiveStatus::NotStarted
            }
            TournamentLiveStatus::RegistrationOpen => {
                infra::repos::tournaments::TournamentLiveStatus::RegistrationOpen
            }
            TournamentLiveStatus::LateRegistration => {
                infra::repos::tournaments::TournamentLiveStatus::LateRegistration
            }
            TournamentLiveStatus::InProgress => {
                infra::repos::tournaments::TournamentLiveStatus::InProgress
            }
            TournamentLiveStatus::Break => infra::repos::tournaments::TournamentLiveStatus::Break,
            TournamentLiveStatus::FinalTable => {
                infra::repos::tournaments::TournamentLiveStatus::FinalTable
            }
            TournamentLiveStatus::Finished => {
                infra::repos::tournaments::TournamentLiveStatus::Finished
            }
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(complex)]
pub struct Tournament {
    pub id: ID,
    pub title: String,
    pub description: Option<String>,
    pub club_id: ID,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub buy_in_cents: i32,
    pub seat_cap: Option<i32>,
    pub status: TournamentStatus, // Calculated: UPCOMING, LIVE, COMPLETED
    pub live_status: TournamentLiveStatus, // Direct from DB: NOT_STARTED, IN_PROGRESS, FINISHED, etc.
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentState {
    pub id: ID,
    pub tournament_id: ID,
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

#[derive(SimpleObject, Clone)]
pub struct Club {
    pub id: ID,
    pub name: String,
    pub city: Option<String>,
}

#[derive(SimpleObject, Clone, serde::Serialize)]
pub struct User {
    pub id: ID,
    pub email: String,
    pub username: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
    pub role: Role,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentRegistration {
    pub id: ID,
    pub tournament_id: ID,
    pub user_id: ID,
    pub registration_time: DateTime<Utc>,
    pub status: String,
    pub notes: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentPlayer {
    pub registration: TournamentRegistration,
    pub user: User,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentResult {
    pub id: ID,
    pub tournament_id: ID,
    pub user_id: ID,
    pub final_position: i32,
    pub prize_cents: i32,
    pub points: i32,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(SimpleObject, Clone)]
pub struct UserTournamentResult {
    pub result: TournamentResult,
    pub tournament: Tournament,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum DealType {
    #[graphql(name = "EVEN_SPLIT")]
    EvenSplit,
    #[graphql(name = "ICM")]
    Icm,
    #[graphql(name = "CUSTOM")]
    Custom,
}

#[derive(SimpleObject, Clone)]
pub struct CustomPayout {
    pub user_id: ID,
    pub amount_cents: i32,
}

#[derive(SimpleObject, Clone)]
pub struct PlayerDeal {
    pub id: ID,
    pub tournament_id: ID,
    pub deal_type: DealType,
    pub affected_positions: Vec<i32>,
    pub custom_payouts: Option<Vec<CustomPayout>>,
    pub total_amount_cents: i32,
    pub notes: Option<String>,
    pub created_by: ID,
}

#[derive(InputObject)]
pub struct EnterTournamentResultsInput {
    pub tournament_id: ID,
    pub payout_template_id: Option<ID>,
    pub player_positions: Vec<PlayerPositionInput>,
    pub deal: Option<PlayerDealInput>,
}

#[derive(InputObject)]
pub struct PlayerPositionInput {
    pub user_id: ID,
    pub final_position: i32,
}

#[derive(InputObject)]
pub struct PlayerDealInput {
    pub deal_type: DealType,
    pub affected_positions: Vec<i32>,
    pub custom_payouts: Option<Vec<CustomPayoutInput>>,
    pub notes: Option<String>,
}

#[derive(InputObject)]
pub struct CustomPayoutInput {
    pub user_id: ID,
    pub amount_cents: i32,
}

#[derive(SimpleObject)]
pub struct EnterTournamentResultsResponse {
    pub success: bool,
    pub results: Vec<TournamentResult>,
    pub deal: Option<PlayerDeal>,
}

#[derive(SimpleObject, Clone)]
pub struct PlayerStatistics {
    pub total_itm: i32,
    pub total_tournaments: i32,
    pub total_winnings: i32,
    pub total_buy_ins: i32,
    pub itm_percentage: f64,
    pub roi_percentage: f64,
}

#[derive(SimpleObject)]
pub struct PlayerStatsResponse {
    pub last_7_days: PlayerStatistics,
    pub last_30_days: PlayerStatistics,
    pub last_year: PlayerStatistics,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentTable {
    pub id: ID,
    pub tournament_id: ID,
    pub table_number: i32,
    pub max_seats: i32,
    pub is_active: bool,
    pub table_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(SimpleObject, Clone)]
pub struct SeatAssignment {
    pub id: ID,
    pub tournament_id: ID,
    pub table_id: ID,
    pub user_id: ID,
    pub seat_number: i32,
    pub stack_size: Option<i32>,
    pub is_current: bool,
    pub assigned_at: DateTime<Utc>,
    pub unassigned_at: Option<DateTime<Utc>>,
    pub assigned_by: Option<ID>,
    pub notes: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct TableWithSeats {
    pub table: TournamentTable,
    pub seats: Vec<SeatWithPlayer>,
}

#[derive(SimpleObject, Clone)]
pub struct SeatWithPlayer {
    pub assignment: SeatAssignment,
    pub player: User,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentSeatingChart {
    pub tournament: Tournament,
    pub tables: Vec<TableWithSeats>,
    pub unassigned_players: Vec<User>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum ClockStatus {
    #[graphql(name = "STOPPED")]
    Stopped,
    #[graphql(name = "RUNNING")]
    Running,
    #[graphql(name = "PAUSED")]
    Paused,
}

impl From<infra::repos::tournament_clock::ClockStatus> for ClockStatus {
    fn from(status: infra::repos::tournament_clock::ClockStatus) -> Self {
        match status {
            infra::repos::tournament_clock::ClockStatus::Stopped => ClockStatus::Stopped,
            infra::repos::tournament_clock::ClockStatus::Running => ClockStatus::Running,
            infra::repos::tournament_clock::ClockStatus::Paused => ClockStatus::Paused,
        }
    }
}

impl From<ClockStatus> for infra::repos::tournament_clock::ClockStatus {
    fn from(status: ClockStatus) -> Self {
        match status {
            ClockStatus::Stopped => infra::repos::tournament_clock::ClockStatus::Stopped,
            ClockStatus::Running => infra::repos::tournament_clock::ClockStatus::Running,
            ClockStatus::Paused => infra::repos::tournament_clock::ClockStatus::Paused,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct TournamentStructure {
    pub id: ID,
    pub tournament_id: ID,
    pub level_number: i32,
    pub small_blind: i32,
    pub big_blind: i32,
    pub ante: i32,
    pub duration_minutes: i32,
    pub is_break: bool,
    pub break_duration_minutes: Option<i32>,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentClock {
    pub id: ID,
    pub tournament_id: ID,
    pub status: ClockStatus,
    pub current_level: i32,
    pub time_remaining_seconds: Option<i64>, // Calculated field
    pub level_started_at: Option<DateTime<Utc>>,
    pub level_end_time: Option<DateTime<Utc>>,
    pub total_pause_duration_seconds: i64, // Calculated field
    pub auto_advance: bool,
    pub current_structure: Option<TournamentStructure>,
}

// Real-time clock update for subscriptions
#[derive(SimpleObject, Clone)]
pub struct ClockUpdate {
    pub tournament_id: ID,
    pub status: ClockStatus,
    pub current_level: i32,
    pub time_remaining_seconds: Option<i64>,
    pub small_blind: i32,
    pub big_blind: i32,
    pub ante: i32,
    pub is_break: bool,
    pub level_duration_minutes: i32,
    pub next_level_preview: Option<TournamentStructure>,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentComplete {
    pub tournament: Tournament,
    pub live_state: Option<TournamentState>,
    pub total_registered: i32,
}

#[derive(SimpleObject, Clone)]
pub struct PlayerRegistrationEvent {
    pub tournament_id: ID,
    pub player: TournamentPlayer,
    pub event_type: String,
}

#[derive(InputObject)]
pub struct RegisterForTournamentInput {
    pub tournament_id: ID,
    pub user_id: Option<ID>, // Optional: if provided, admin can register another user
    pub notes: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct AuthPayload {
    pub token: String,
    pub user: User,
}

#[derive(SimpleObject, Clone)]
pub struct OAuthUrlResponse {
    pub auth_url: String,
    pub csrf_token: String,
}

#[derive(InputObject)]
pub struct OAuthCallbackInput {
    pub provider: String,
    pub code: String,
    pub csrf_token: String,
}

#[derive(SimpleObject, Clone)]
pub struct OAuthClient {
    pub id: ID,
    pub client_id: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub is_active: bool,
}

#[derive(InputObject)]
pub struct CreateOAuthClientInput {
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Option<Vec<String>>,
}

#[derive(SimpleObject)]
pub struct CreateOAuthClientResponse {
    pub client: OAuthClient,
    pub client_secret: String,
}

#[derive(InputObject)]
pub struct UserRegistrationInput {
    pub email: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub username: Option<String>,
}

#[derive(InputObject)]
pub struct UserLoginInput {
    pub email: String,
    pub password: String,
}

#[derive(InputObject)]
pub struct CreateTournamentTableInput {
    pub tournament_id: ID,
    pub table_number: i32,
    pub max_seats: Option<i32>,
    pub table_name: Option<String>,
}

#[derive(InputObject)]
pub struct AssignPlayerToSeatInput {
    pub tournament_id: ID,
    pub table_id: ID,
    pub user_id: ID,
    pub seat_number: i32,
    pub stack_size: Option<i32>,
    pub notes: Option<String>,
}

#[derive(InputObject)]
pub struct MovePlayerInput {
    pub tournament_id: ID,
    pub user_id: ID,
    pub new_table_id: ID,
    pub new_seat_number: i32,
    pub notes: Option<String>,
}

#[derive(InputObject)]
pub struct UpdateStackSizeInput {
    pub tournament_id: ID,
    pub user_id: ID,
    pub new_stack_size: i32,
}

#[derive(InputObject)]
pub struct UpdateTournamentStatusInput {
    pub tournament_id: ID,
    pub live_status: TournamentLiveStatus,
}

#[derive(InputObject)]
pub struct UpdateTournamentStateInput {
    pub tournament_id: ID,
    pub current_level: Option<i32>,
    pub players_remaining: Option<i32>,
    pub break_until: Option<DateTime<Utc>>,
    pub current_small_blind: Option<i32>,
    pub current_big_blind: Option<i32>,
    pub current_ante: Option<i32>,
    pub level_started_at: Option<DateTime<Utc>>,
    pub level_duration_minutes: Option<i32>,
}

#[derive(InputObject)]
pub struct BalanceTablesInput {
    pub tournament_id: ID,
    pub target_players_per_table: Option<i32>,
}

#[derive(SimpleObject, Clone)]
pub struct SeatingChangeEvent {
    pub event_type: SeatingEventType,
    pub tournament_id: ID,
    pub club_id: ID, // Add club_id to enable club-based filtering
    pub affected_assignment: Option<SeatAssignment>,
    pub affected_player: Option<User>,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum SeatingEventType {
    #[graphql(name = "PLAYER_ASSIGNED")]
    PlayerAssigned,
    #[graphql(name = "PLAYER_MOVED")]
    PlayerMoved,
    #[graphql(name = "PLAYER_ELIMINATED")]
    PlayerEliminated,
    #[graphql(name = "STACK_UPDATED")]
    StackUpdated,
    #[graphql(name = "TABLE_CREATED")]
    TableCreated,
    #[graphql(name = "TABLE_CLOSED")]
    TableClosed,
    #[graphql(name = "TOURNAMENT_STATUS_CHANGED")]
    TournamentStatusChanged,
    #[graphql(name = "TABLES_BALANCED")]
    TablesBalanced,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum LeaderboardPeriod {
    #[graphql(name = "ALL_TIME")]
    AllTime,
    #[graphql(name = "LAST_YEAR")]
    LastYear,
    #[graphql(name = "LAST_6_MONTHS")]
    Last6Months,
    #[graphql(name = "LAST_30_DAYS")]
    Last30Days,
    #[graphql(name = "LAST_7_DAYS")]
    Last7Days,
}

impl From<LeaderboardPeriod> for infra::repos::LeaderboardPeriod {
    fn from(period: LeaderboardPeriod) -> Self {
        match period {
            LeaderboardPeriod::AllTime => infra::repos::LeaderboardPeriod::AllTime,
            LeaderboardPeriod::LastYear => infra::repos::LeaderboardPeriod::LastYear,
            LeaderboardPeriod::Last6Months => infra::repos::LeaderboardPeriod::Last6Months,
            LeaderboardPeriod::Last30Days => infra::repos::LeaderboardPeriod::Last30Days,
            LeaderboardPeriod::Last7Days => infra::repos::LeaderboardPeriod::Last7Days,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct LeaderboardEntry {
    pub user: User, // Full user object with complete info
    pub rank: i32,  // Position in leaderboard (1-based)
    pub total_tournaments: i32,
    pub total_buy_ins: i32,  // Total amount spent (cents)
    pub total_winnings: i32, // Total amount won (cents)
    pub net_profit: i32,     // winnings - buy_ins (cents)
    pub total_itm: i32,      // Number of tournaments where player finished in the money
    pub itm_percentage: f64, // (total_itm / total_tournaments) * 100
    pub roi_percentage: f64, // ((total_winnings - total_buy_ins) / total_buy_ins) * 100
    pub average_finish: f64, // Average finishing position
    pub first_places: i32,   // Number of first place finishes
    pub final_tables: i32,   // Number of final table finishes (top 9)
    pub points: f64,         // Calculated leaderboard points
}

#[derive(SimpleObject)]
pub struct LeaderboardResponse {
    pub entries: Vec<LeaderboardEntry>,
    pub total_players: i32,
    pub period: LeaderboardPeriod,
}

#[ComplexObject]
impl Tournament {
    async fn club(&self, ctx: &Context<'_>) -> Result<Club> {
        let loader = ctx.data::<DataLoader<ClubLoader>>()?;
        let club_uuid =
            Uuid::parse_str(self.club_id.as_str()).map_err(|e| Error::new(e.to_string()))?;

        match loader
            .load_one(club_uuid)
            .await
            .map_err(|e| Error::new(e.to_string()))?
        {
            Some(row) => Ok(Club {
                id: row.id.into(),
                name: row.name,
                city: row.city,
            }),
            None => Err(Error::new("Club not found")),
        }
    }
}
