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
    pub created_at: DateTime<Utc>,
}

#[derive(SimpleObject, Clone)]
pub struct ClubTable {
    pub id: ID,
    pub club_id: ID,
    pub table_number: i32,
    pub max_seats: i32,
    pub is_active: bool,
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
    #[graphql(name = "REGISTERED")]
    Registered,

    /// Player has checked in and is ready to be seated
    #[graphql(name = "CHECKED_IN")]
    CheckedIn,

    /// Player is seated and actively playing
    #[graphql(name = "SEATED")]
    Seated,

    /// Player was eliminated/busted from tournament
    #[graphql(name = "BUSTED")]
    Busted,

    /// Player was placed on waiting list (tournament full)
    #[graphql(name = "WAITLISTED")]
    Waitlisted,

    /// Player cancelled their registration
    #[graphql(name = "CANCELLED")]
    Cancelled,

    /// Player didn't show up for check-in
    #[graphql(name = "NO_SHOW")]
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
pub struct UpdateTournamentStatusInput {
    pub tournament_id: ID,
    pub live_status: TournamentLiveStatus,
}

#[derive(InputObject)]
pub struct BalanceTablesInput {
    pub tournament_id: ID,
    pub target_players_per_table: Option<i32>,
}

#[derive(InputObject)]
pub struct CheckInPlayerInput {
    pub tournament_id: ID,
    pub user_id: ID,
}

#[derive(InputObject)]
pub struct UpdateRegistrationStatusInput {
    pub tournament_id: ID,
    pub user_id: ID,
    pub status: RegistrationStatus,
    pub notes: Option<String>,
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

    async fn structure(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<TournamentStructure>> {
        use crate::state::AppState;
        use infra::repos::TournamentClockRepo;

        let state = ctx.data::<AppState>()?;
        let clock_repo = TournamentClockRepo::new(state.db.clone());

        let tournament_id = uuid::Uuid::parse_str(self.id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament ID: {}", e)))?;

        let structures = clock_repo.get_all_structures(tournament_id).await?;

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
        use infra::repos::{ClockStatus as InfraClockStatus, TournamentClockRepo};
        use std::str::FromStr;

        let state = ctx.data::<AppState>()?;
        let clock_repo = TournamentClockRepo::new(state.db.clone());

        let tournament_id = uuid::Uuid::parse_str(self.id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament ID: {}", e)))?;

        let clock = clock_repo.get_clock(tournament_id).await?;

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
        use infra::repos::TournamentRegistrationRepo;

        let state = ctx.data::<AppState>()?;
        let registration_repo = TournamentRegistrationRepo::new(state.db.clone());

        let tournament_id = uuid::Uuid::parse_str(self.id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament ID: {}", e)))?;

        let registrations = registration_repo.get_by_tournament(tournament_id).await?;

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
        use crate::state::AppState;
        use infra::repos::UserRepo;

        let state = ctx.data::<AppState>()?;
        let user_repo = UserRepo::new(state.db.clone());

        let user_id = uuid::Uuid::parse_str(self.user_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;

        let user_row = user_repo.get_by_id(user_id).await?;

        Ok(user_row.map(|user| User {
            id: user.id.into(),
            email: user.email,
            username: user.username,
            first_name: user.first_name,
            last_name: user.last_name,
            phone: user.phone,
            is_active: user.is_active,
            role: Role::from(user.role),
        }))
    }
}

#[ComplexObject]
impl User {
    async fn managed_club(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Club>> {
        use crate::state::AppState;
        use infra::repos::{ClubManagerRepo, ClubRepo};

        let state = ctx.data::<AppState>()?;
        let club_manager_repo = ClubManagerRepo::new(state.db.clone());
        let club_repo = ClubRepo::new(state.db.clone());

        let user_id = uuid::Uuid::parse_str(self.id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;

        let managed_clubs = club_manager_repo.get_manager_clubs(user_id).await?;

        // Get the first managed club if any
        if let Some(club_info) = managed_clubs.into_iter().next() {
            let club_row = club_repo.get(club_info.club_id).await?;
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
