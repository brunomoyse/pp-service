use async_graphql::{Enum, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};

// Notification title constants
pub const TITLE_REGISTRATION_CONFIRMED: &str = "Registration Confirmed";
pub const TITLE_TOURNAMENT_STARTING: &str = "Tournament Starting Soon";
pub const TITLE_WAITLIST_PROMOTED: &str = "Waitlist Promoted";
pub const TITLE_WAITLISTED: &str = "Added to Waitlist";

// Pagination types

#[derive(InputObject, Debug, Clone)]
pub struct PaginationInput {
    /// Number of items per page (1-200, default: 50)
    pub limit: Option<i32>,
    /// Number of items to skip (default: 0)
    pub offset: Option<i32>,
}

impl PaginationInput {
    /// Convert to infra LimitOffset with validation
    pub fn to_limit_offset(&self) -> infra::pagination::LimitOffset {
        infra::pagination::LimitOffset {
            limit: self.limit.unwrap_or(50).clamp(1, 200) as i64,
            offset: self.offset.unwrap_or(0).max(0) as i64,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(concrete(name = "PaginatedUsers", params(crate::gql::types::User)))]
#[graphql(concrete(name = "PaginatedTournaments", params(crate::gql::types::Tournament)))]
#[graphql(concrete(
    name = "PaginatedTournamentPlayers",
    params(crate::gql::types::TournamentPlayer)
))]
#[graphql(concrete(
    name = "PaginatedLeaderboard",
    params(crate::gql::types::LeaderboardEntry)
))]
#[graphql(concrete(
    name = "PaginatedActivityLog",
    params(crate::gql::types::ActivityLogEntry)
))]
pub struct PaginatedResponse<T: async_graphql::OutputType> {
    /// List of items for the current page
    pub items: Vec<T>,
    /// Total number of items across all pages
    pub total_count: i32,
    /// Number of items in the current page
    pub page_size: i32,
    /// Current offset
    pub offset: i32,
    /// Whether there are more items beyond this page
    pub has_next_page: bool,
}

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

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum NotificationType {
    TournamentStartingSoon,
    RegistrationConfirmed,
    WaitlistPromoted,
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
