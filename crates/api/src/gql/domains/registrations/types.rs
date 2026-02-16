use async_graphql::dataloader::DataLoader;
use async_graphql::{ComplexObject, Context, Enum, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::gql::domains::seating::types::SeatAssignment;
use crate::gql::domains::users::types::User;
use crate::gql::error::ResultExt;
use crate::gql::loaders::UserLoader;
use crate::state::AppState;

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

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum RegistrationEventType {
    PlayerRegistered,
    PlayerUnregistered,
    PlayerWaitlisted,
    PlayerPromoted,
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

impl From<infra::models::TournamentRegistrationRow> for TournamentRegistration {
    fn from(row: infra::models::TournamentRegistrationRow) -> Self {
        Self {
            id: row.id.into(),
            tournament_id: row.tournament_id.into(),
            user_id: row.user_id.into(),
            registration_time: row.registration_time,
            status: row.status.into(),
            notes: row.notes,
        }
    }
}

#[ComplexObject]
impl TournamentRegistration {
    async fn user(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<User>> {
        let user_id = Uuid::parse_str(self.user_id.as_str()).gql_err("Invalid user ID")?;

        let loader = ctx.data::<DataLoader<UserLoader>>()?;

        match loader
            .load_one(user_id)
            .await
            .gql_err("Loading user failed")?
        {
            Some(user) => Ok(Some(user.into())),
            None => Ok(None),
        }
    }

    /// Returns the player's position in the waitlist (1-based). Null if not waitlisted.
    async fn waitlist_position(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<i32>> {
        if self.status != RegistrationStatus::Waitlisted {
            return Ok(None);
        }

        let state = ctx.data::<AppState>()?;
        let tournament_id =
            Uuid::parse_str(self.tournament_id.as_str()).gql_err("Invalid tournament ID")?;
        let user_id = Uuid::parse_str(self.user_id.as_str()).gql_err("Invalid user ID")?;

        let position = infra::repos::tournament_registrations::get_waitlist_position(
            &state.db,
            tournament_id,
            user_id,
        )
        .await
        .gql_err("Failed to get waitlist position")?;

        Ok(position.map(|p| p as i32))
    }
}

#[derive(SimpleObject, Clone)]
pub struct TournamentPlayer {
    pub registration: TournamentRegistration,
    pub user: User,
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

#[derive(InputObject)]
pub struct CancelRegistrationInput {
    pub tournament_id: ID,
    pub user_id: ID,
}

#[derive(SimpleObject)]
pub struct CancelRegistrationResponse {
    pub registration: TournamentRegistration,
    pub promoted_player: Option<TournamentPlayer>,
}
