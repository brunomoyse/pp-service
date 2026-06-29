use async_graphql::{Enum, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};

use crate::gql::domains::tournaments::types::Tournament;
use crate::gql::domains::users::types::User;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
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

#[derive(SimpleObject, Clone)]
pub struct TournamentTable {
    pub id: ID,
    pub tournament_id: ID,
    pub table_number: i32,
    pub max_seats: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(SimpleObject, Clone, serde::Serialize, serde::Deserialize)]
pub struct SeatAssignment {
    pub id: ID,
    pub tournament_id: ID,
    pub club_table_id: ID,
    /// The app user, when this player has an account. Null for account-less players.
    pub user_id: Option<ID>,
    /// The club roster identity — always present.
    pub club_player_id: ID,
    pub seat_number: i32,
    pub stack_size: Option<i32>,
    pub is_current: bool,
    pub assigned_at: DateTime<Utc>,
    pub unassigned_at: Option<DateTime<Utc>>,
    pub assigned_by: Option<ID>,
    pub notes: Option<String>,
}

impl From<infra::models::TableSeatAssignmentRow> for SeatAssignment {
    fn from(row: infra::models::TableSeatAssignmentRow) -> Self {
        Self {
            id: row.id.into(),
            tournament_id: row.tournament_id.into(),
            club_table_id: row.club_table_id.into(),
            user_id: row.user_id.map(Into::into),
            club_player_id: row.club_player_id.into(),
            seat_number: row.seat_number,
            stack_size: row.stack_size,
            is_current: row.is_current,
            assigned_at: row.assigned_at,
            unassigned_at: row.unassigned_at,
            assigned_by: row.assigned_by.map(|id| id.into()),
            notes: row.notes,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct TableWithSeats {
    pub table: TournamentTable,
    pub seats: Vec<SeatWithPlayer>,
}

#[derive(SimpleObject, Clone)]
pub struct SeatWithPlayer {
    pub assignment: SeatAssignment,
    /// Display name of the seated player (works for account-less players).
    pub display_name: String,
    /// The app user, when the player has an account.
    pub player: Option<User>,
}

/// A player who is registered/checked-in but not currently seated.
#[derive(SimpleObject, Clone)]
pub struct UnseatedPlayer {
    pub club_player_id: ID,
    pub display_name: String,
    pub user: Option<User>,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentSeatingChart {
    pub tournament: Tournament,
    pub tables: Vec<TableWithSeats>,
    pub unassigned_players: Vec<UnseatedPlayer>,
    /// Active tables are unbalanced beyond the TDA threshold (within 1 player
    /// for <= 6 tables, within 2 above) — the manager should rebalance.
    pub needs_rebalancing: bool,
    /// The field now fits on fewer tables than are in use — consolidate.
    pub needs_consolidation: bool,
    /// Player spread is 3+ — play should halt on the short table until balanced.
    pub balance_critical: bool,
    /// Minimal number of tables the current field should occupy.
    pub suggested_table_count: i32,
}

#[derive(SimpleObject, Clone, serde::Serialize, serde::Deserialize)]
pub struct SeatingChangeEvent {
    pub event_type: SeatingEventType,
    pub tournament_id: ID,
    pub club_id: ID, // Add club_id to enable club-based filtering
    pub affected_assignment: Option<SeatAssignment>,
    pub affected_player: Option<User>,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

// Input types

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
    /// The app user to seat. Provide this OR `club_player_id`.
    pub user_id: Option<ID>,
    /// The club roster identity to seat, for account-less players. Provide this
    /// OR `user_id`.
    pub club_player_id: Option<ID>,
    pub seat_number: i32,
    pub stack_size: Option<i32>,
    pub notes: Option<String>,
}

/// Seat a single checked-in roster player on a random free seat. Identified by
/// the club roster id so it works for account-less players too.
#[derive(InputObject)]
pub struct AutoSeatPlayerInput {
    pub tournament_id: ID,
    pub club_player_id: ID,
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
    pub max_seats: Option<i32>,
}

#[derive(InputObject)]
pub struct BulkAssignTableEntry {
    pub club_table_id: ID,
    pub max_seats: Option<i32>,
}

#[derive(InputObject)]
pub struct AssignTablesToTournamentInput {
    pub tournament_id: ID,
    pub tables: Vec<BulkAssignTableEntry>,
}

#[derive(InputObject)]
pub struct UnassignTableFromTournamentInput {
    pub tournament_id: ID,
    pub club_table_id: ID,
}

#[derive(InputObject)]
pub struct BalanceTablesInput {
    pub tournament_id: ID,
    pub target_players_per_table: Option<i32>,
}

/// A single recorded knockout in a bounty / PKO tournament.
#[derive(SimpleObject)]
pub struct TournamentBounty {
    pub id: ID,
    pub tournament_id: ID,
    pub hunter_club_player_id: ID,
    pub victim_club_player_id: ID,
    pub hunter_name: String,
    pub victim_name: String,
    /// Cash the hunter collected for this knockout, in cents.
    pub amount_cents: i32,
    pub created_at: DateTime<Utc>,
}
