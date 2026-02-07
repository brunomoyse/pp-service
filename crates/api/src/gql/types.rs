use async_graphql::dataloader::DataLoader;
use async_graphql::{ComplexObject, Context, Enum, Error, InputObject, Result, SimpleObject, ID};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::gql::error::ResultExt;
use crate::gql::loaders::ClubLoader;

// Notification title constants
pub const TITLE_REGISTRATION_CONFIRMED: &str = "Registration Confirmed";
pub const TITLE_TOURNAMENT_STARTING: &str = "Tournament Starting Soon";

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Role {
    Admin,
    Manager,
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
    Upcoming,
    InProgress,
    Completed,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum TournamentLiveStatus {
    NotStarted,
    RegistrationOpen,
    LateRegistration,
    InProgress,
    Break,
    FinalTable,
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
    pub early_bird_bonus_chips: Option<i32>, // Extra chips for players present at tournament start
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
#[graphql(complex)]
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
#[graphql(complex)]
pub struct TournamentRegistration {
    pub id: ID,
    pub tournament_id: ID,
    pub user_id: ID,
    pub registration_time: DateTime<Utc>,
    pub status: RegistrationStatus,
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
    EvenSplit,
    Icm,
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
    pub created_at: DateTime<Utc>,
}

#[derive(SimpleObject, Clone)]
pub struct ClubTable {
    pub id: ID,
    pub club_id: ID,
    pub table_number: i32,
    pub max_seats: i32,
    pub is_active: bool,
    pub is_assigned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(SimpleObject, Clone)]
pub struct SeatAssignment {
    pub id: ID,
    pub tournament_id: ID,
    pub club_table_id: ID,
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

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum RegistrationStatus {
    /// Player has registered but not yet checked in
    Registered,

    /// Player has checked in and is ready to be seated
    CheckedIn,

    /// Player is seated and actively playing
    Seated,

    /// Player was eliminated/busted from tournament
    Busted,

    /// Player was placed on waiting list (tournament full)
    Waitlisted,

    /// Player cancelled their registration
    Cancelled,

    /// Player didn't show up for check-in
    NoShow,
}

impl From<String> for RegistrationStatus {
    fn from(status: String) -> Self {
        match status.as_str() {
            "registered" => RegistrationStatus::Registered,
            "checked_in" => RegistrationStatus::CheckedIn,
            "seated" => RegistrationStatus::Seated,
            "busted" => RegistrationStatus::Busted,
            "waitlisted" => RegistrationStatus::Waitlisted,
            "cancelled" => RegistrationStatus::Cancelled,
            "no_show" => RegistrationStatus::NoShow,
            _ => RegistrationStatus::Registered, // Default fallback
        }
    }
}

impl From<RegistrationStatus> for String {
    fn from(status: RegistrationStatus) -> Self {
        match status {
            RegistrationStatus::Registered => "registered".to_string(),
            RegistrationStatus::CheckedIn => "checked_in".to_string(),
            RegistrationStatus::Seated => "seated".to_string(),
            RegistrationStatus::Busted => "busted".to_string(),
            RegistrationStatus::Waitlisted => "waitlisted".to_string(),
            RegistrationStatus::Cancelled => "cancelled".to_string(),
            RegistrationStatus::NoShow => "no_show".to_string(),
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum ClockStatus {
    Stopped,
    Running,
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

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum EntryType {
    Initial,
    Rebuy,
    ReEntry,
    Addon,
}

impl From<String> for EntryType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "initial" => EntryType::Initial,
            "rebuy" => EntryType::Rebuy,
            "re_entry" => EntryType::ReEntry,
            "addon" => EntryType::Addon,
            _ => EntryType::Initial,
        }
    }
}

impl From<EntryType> for String {
    fn from(e: EntryType) -> Self {
        match e {
            EntryType::Initial => "initial".to_string(),
            EntryType::Rebuy => "rebuy".to_string(),
            EntryType::ReEntry => "re_entry".to_string(),
            EntryType::Addon => "addon".to_string(),
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct TournamentEntry {
    pub id: ID,
    pub tournament_id: ID,
    pub user_id: ID,
    pub entry_type: EntryType,
    pub amount_cents: i32,
    pub chips_received: Option<i32>,
    pub recorded_by: Option<ID>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentEntryStats {
    pub tournament_id: ID,
    pub total_entries: i32,
    pub total_amount_cents: i32,
    pub unique_players: i32,
    pub initial_count: i32,
    pub rebuy_count: i32,
    pub re_entry_count: i32,
    pub addon_count: i32,
}

#[derive(InputObject)]
pub struct AddTournamentEntryInput {
    pub tournament_id: ID,
    pub user_id: ID,
    pub entry_type: EntryType,
    pub amount_cents: Option<i32>,
    pub chips_received: Option<i32>,
    pub notes: Option<String>,
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
    pub next_structure: Option<TournamentStructure>,
    // Additional fields for real-time updates (previously in ClockUpdate)
    pub small_blind: Option<i32>,
    pub big_blind: Option<i32>,
    pub ante: Option<i32>,
    pub is_break: Option<bool>,
    pub level_duration_minutes: Option<i32>,
}

#[derive(SimpleObject, Clone)]
pub struct PayoutPosition {
    pub position: i32,
    pub percentage: f64,
    pub amount_cents: i32,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentPayout {
    pub id: ID,
    pub tournament_id: ID,
    pub template_id: Option<ID>,
    pub player_count: i32,
    pub total_prize_pool: i32,
    pub positions: Vec<PayoutPosition>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum RegistrationEventType {
    PlayerRegistered,
    PlayerUnregistered,
}

#[derive(SimpleObject, Clone)]
pub struct PlayerRegistrationEvent {
    pub tournament_id: ID,
    pub player: TournamentPlayer,
    pub event_type: RegistrationEventType,
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
}

#[derive(InputObject)]
pub struct AssignPlayerToSeatInput {
    pub tournament_id: ID,
    pub club_table_id: ID,
    pub user_id: ID,
    pub seat_number: i32,
    pub stack_size: Option<i32>,
    pub notes: Option<String>,
}

#[derive(InputObject)]
pub struct MovePlayerInput {
    pub tournament_id: ID,
    pub user_id: ID,
    pub new_club_table_id: ID,
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
pub struct AssignTableToTournamentInput {
    pub tournament_id: ID,
    pub club_table_id: ID,
}

#[derive(InputObject)]
pub struct UnassignTableFromTournamentInput {
    pub tournament_id: ID,
    pub club_table_id: ID,
}

#[derive(InputObject)]
pub struct UpdateTournamentStatusInput {
    pub tournament_id: ID,
    pub live_status: TournamentLiveStatus,
}

#[derive(InputObject)]
pub struct BalanceTablesInput {
    pub tournament_id: ID,
    pub target_players_per_table: Option<i32>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum AssignmentStrategy {
    /// Balanced distribution - fills tables evenly
    Balanced,

    /// Random assignment for fairness
    Random,

    /// Sequential - fills tables in order
    Sequential,

    /// Manual - no auto-assignment
    Manual,
}

#[derive(InputObject)]
pub struct CheckInPlayerInput {
    pub tournament_id: ID,
    pub user_id: ID,
    pub assignment_strategy: Option<AssignmentStrategy>,
    pub auto_assign: Option<bool>,            // Default true
    pub grant_early_bird_bonus: Option<bool>, // Manually grant early bird bonus on late check-in
}

#[derive(InputObject)]
pub struct UpdateRegistrationStatusInput {
    pub tournament_id: ID,
    pub user_id: ID,
    pub status: RegistrationStatus,
    pub notes: Option<String>,
}

#[derive(SimpleObject)]
pub struct CheckInResponse {
    pub registration: TournamentRegistration,
    pub seat_assignment: Option<SeatAssignment>,
    pub message: String,
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
    PlayerAssigned,
    PlayerMoved,
    PlayerEliminated,
    StackUpdated,
    TableCreated,
    TableClosed,
    TableRemoved,
    TournamentStatusChanged,
    TablesBalanced,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum NotificationType {
    TournamentStartingSoon,
    RegistrationConfirmed,
    TournamentStatusChanged,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct UserNotification {
    pub id: ID,
    pub user_id: ID,
    pub notification_type: NotificationType,
    pub title: String,
    pub message: String,
    pub tournament_id: Option<ID>,
    pub created_at: DateTime<Utc>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum LeaderboardPeriod {
    AllTime,
    LastYear,
    Last6Months,
    Last30Days,
    Last7Days,
}

impl From<LeaderboardPeriod> for infra::repos::tournament_results::LeaderboardPeriod {
    fn from(period: LeaderboardPeriod) -> Self {
        match period {
            LeaderboardPeriod::AllTime => {
                infra::repos::tournament_results::LeaderboardPeriod::AllTime
            }
            LeaderboardPeriod::LastYear => {
                infra::repos::tournament_results::LeaderboardPeriod::LastYear
            }
            LeaderboardPeriod::Last6Months => {
                infra::repos::tournament_results::LeaderboardPeriod::Last6Months
            }
            LeaderboardPeriod::Last30Days => {
                infra::repos::tournament_results::LeaderboardPeriod::Last30Days
            }
            LeaderboardPeriod::Last7Days => {
                infra::repos::tournament_results::LeaderboardPeriod::Last7Days
            }
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
        let club_uuid = Uuid::parse_str(self.club_id.as_str()).gql_err("Invalid club ID")?;

        match loader
            .load_one(club_uuid)
            .await
            .gql_err("Loading club failed")?
        {
            Some(row) => Ok(Club {
                id: row.id.into(),
                name: row.name,
                city: row.city,
            }),
            None => Err(Error::new("Club not found")),
        }
    }

    async fn structure(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<TournamentStructure>> {
        use crate::state::AppState;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            uuid::Uuid::parse_str(self.id.as_str()).gql_err("Invalid tournament ID")?;

        let structures =
            infra::repos::tournament_clock::get_all_structures(&state.db, tournament_id).await?;

        Ok(structures
            .into_iter()
            .map(|structure| TournamentStructure {
                id: structure.id.into(),
                tournament_id: structure.tournament_id.into(),
                level_number: structure.level_number,
                small_blind: structure.small_blind,
                big_blind: structure.big_blind,
                ante: structure.ante,
                duration_minutes: structure.duration_minutes,
                is_break: structure.is_break,
                break_duration_minutes: structure.break_duration_minutes,
            })
            .collect())
    }

    async fn clock(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<TournamentClock>> {
        use crate::state::AppState;
        use infra::repos::tournament_clock::ClockStatus as InfraClockStatus;
        use std::str::FromStr;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            uuid::Uuid::parse_str(self.id.as_str()).gql_err("Invalid tournament ID")?;

        let clock = infra::repos::tournament_clock::get_clock(&state.db, tournament_id).await?;

        Ok(clock.map(|clock_row| TournamentClock {
            id: clock_row.id.into(),
            tournament_id: clock_row.tournament_id.into(),
            status: InfraClockStatus::from_str(&clock_row.clock_status)
                .unwrap_or(InfraClockStatus::Stopped)
                .into(),
            current_level: clock_row.current_level,
            time_remaining_seconds: None, // This would need calculation
            level_started_at: clock_row.level_started_at,
            level_end_time: clock_row.level_end_time,
            total_pause_duration_seconds: clock_row.total_pause_duration.microseconds / 1_000_000,
            auto_advance: clock_row.auto_advance,
            current_structure: None, // These would require additional queries
            next_structure: None,
            small_blind: None,
            big_blind: None,
            ante: None,
            is_break: None,
            level_duration_minutes: None,
        }))
    }

    async fn registrations(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<TournamentRegistration>> {
        use crate::state::AppState;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            uuid::Uuid::parse_str(self.id.as_str()).gql_err("Invalid tournament ID")?;

        let registrations =
            infra::repos::tournament_registrations::list_by_tournament(&state.db, tournament_id)
                .await?;

        Ok(registrations
            .into_iter()
            .map(|registration| TournamentRegistration {
                id: registration.id.into(),
                tournament_id: registration.tournament_id.into(),
                user_id: registration.user_id.into(),
                registration_time: registration.registration_time,
                status: registration.status.into(),
                notes: registration.notes,
            })
            .collect())
    }
}

#[ComplexObject]
impl TournamentRegistration {
    async fn user(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<User>> {
        use crate::gql::loaders::UserLoader;

        let user_id = Uuid::parse_str(self.user_id.as_str()).gql_err("Invalid user ID")?;

        let loader = ctx.data::<DataLoader<UserLoader>>()?;

        match loader
            .load_one(user_id)
            .await
            .gql_err("Loading user failed")?
        {
            Some(user) => Ok(Some(User {
                id: user.id.into(),
                email: user.email,
                username: user.username,
                first_name: user.first_name,
                last_name: user.last_name,
                phone: user.phone,
                is_active: user.is_active,
                role: Role::from(user.role),
            })),
            None => Ok(None),
        }
    }
}

#[ComplexObject]
impl User {
    async fn managed_club(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Club>> {
        use crate::state::AppState;

        let state = ctx.data::<AppState>()?;

        let user_id = uuid::Uuid::parse_str(self.id.as_str()).gql_err("Invalid user ID")?;

        let managed_clubs =
            infra::repos::club_managers::get_manager_clubs(&state.db, user_id).await?;

        // Get the first managed club if any
        if let Some(club_info) = managed_clubs.into_iter().next() {
            let club_row = infra::repos::clubs::get_by_id(&state.db, club_info.club_id).await?;
            Ok(club_row.map(|club| Club {
                id: club.id.into(),
                name: club.name,
                city: club.city,
            }))
        } else {
            Ok(None)
        }
    }
}

// Player management input types
#[derive(InputObject)]
pub struct CreatePlayerInput {
    pub email: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub phone: Option<String>,
    pub club_id: ID,
}

#[derive(InputObject)]
pub struct UpdatePlayerInput {
    pub id: ID,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub phone: Option<String>,
}

// Tournament management input types
#[derive(InputObject)]
pub struct CreateTournamentInput {
    pub club_id: ID,
    pub name: String,
    pub description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub buy_in_cents: i32,
    pub seat_cap: Option<i32>,
    pub early_bird_bonus_chips: Option<i32>,
    /// Blind structure template ID - if provided, copies levels from template
    pub template_id: Option<ID>,
    /// Custom blind structure levels - only used if template_id is not provided
    pub structure: Option<Vec<TournamentStructureInput>>,
}

#[derive(InputObject)]
pub struct UpdateTournamentInput {
    pub id: ID,
    pub name: Option<String>,
    pub description: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub buy_in_cents: Option<i32>,
    pub seat_cap: Option<i32>,
    pub early_bird_bonus_chips: Option<i32>,
    /// Blind structure template ID - if provided, replaces structure with template levels
    pub template_id: Option<ID>,
    /// Custom blind structure levels - only used if template_id is not provided
    pub structure: Option<Vec<TournamentStructureInput>>,
}

#[derive(InputObject, Clone)]
pub struct TournamentStructureInput {
    pub level_number: i32,
    pub small_blind: i32,
    pub big_blind: i32,
    pub ante: i32,
    pub duration_minutes: i32,
    pub is_break: bool,
    pub break_duration_minutes: Option<i32>,
}
