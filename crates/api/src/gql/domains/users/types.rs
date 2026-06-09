use async_graphql::{ComplexObject, Context, InputObject, SimpleObject, ID};

use crate::gql::common::types::Role;
use crate::gql::domains::clubs::types::Club;
use crate::gql::error::ResultExt;

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
    pub locale: String,
}

impl From<infra::models::UserRow> for User {
    fn from(row: infra::models::UserRow) -> Self {
        Self {
            id: row.id.into(),
            email: row.email,
            username: row.username,
            first_name: row.first_name,
            last_name: row.last_name,
            phone: row.phone,
            is_active: row.is_active,
            role: Role::from(row.role),
            locale: row.locale,
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
            Ok(club_row.map(Club::from))
        } else {
            Ok(None)
        }
    }

    /// Whether this user currently holds an active Pro entitlement.
    async fn is_pro(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        use crate::state::AppState;

        let state = ctx.data::<AppState>()?;
        let user_id = uuid::Uuid::parse_str(self.id.as_str()).gql_err("Invalid user ID")?;
        Ok(infra::repos::pro_entitlements::is_pro(&state.db, user_id).await?)
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

/// Per-category notification preferences for the current user. Defaults are
/// all on until the user changes something.
#[derive(SimpleObject, Clone, Copy)]
pub struct NotificationPreferences {
    pub tournament_reminders: bool,
    pub registration_updates: bool,
    pub seating_updates: bool,
    pub achievements: bool,
}

impl From<infra::repos::notification_preferences::NotificationPreferences>
    for NotificationPreferences
{
    fn from(p: infra::repos::notification_preferences::NotificationPreferences) -> Self {
        Self {
            tournament_reminders: p.tournament_reminders,
            registration_updates: p.registration_updates,
            seating_updates: p.seating_updates,
            achievements: p.achievements,
        }
    }
}

/// Partial update: omitted fields keep their current value.
#[derive(InputObject)]
pub struct UpdateNotificationPreferencesInput {
    pub tournament_reminders: Option<bool>,
    pub registration_updates: Option<bool>,
    pub seating_updates: Option<bool>,
    pub achievements: Option<bool>,
}
