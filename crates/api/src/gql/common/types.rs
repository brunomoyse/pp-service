use async_graphql::{Enum, SimpleObject, ID};
use chrono::{DateTime, Utc};

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
