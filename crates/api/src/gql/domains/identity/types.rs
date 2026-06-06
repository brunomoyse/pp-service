use async_graphql::{ComplexObject, Context, InputObject, SimpleObject, ID};

use crate::gql::domains::clubs::types::Club;
use crate::gql::error::ResultExt;
use crate::state::AppState;

/// A club roster entry. Exists for everyone a club has registered, whether or
/// not they are an onboarded app user. `app_user_id` is set once claimed.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct RegisteredPlayer {
    pub id: ID,
    pub club_id: ID,
    pub display_name: String,
    /// The linked app user, set once the roster entry has been claimed.
    pub app_user_id: Option<ID>,
    /// Whether this roster entry is linked to an onboarded app user.
    pub is_claimed: bool,
}

impl From<infra::models::RegisteredPlayerRow> for RegisteredPlayer {
    fn from(row: infra::models::RegisteredPlayerRow) -> Self {
        let is_claimed = row.app_user_id.is_some();
        Self {
            id: row.id.into(),
            club_id: row.club_id.into(),
            display_name: row.display_name,
            app_user_id: row.app_user_id.map(Into::into),
            is_claimed,
        }
    }
}

#[ComplexObject]
impl RegisteredPlayer {
    /// The club this roster entry belongs to.
    async fn club(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Club>> {
        let state = ctx.data::<AppState>()?;
        let club_id = uuid::Uuid::parse_str(self.club_id.as_str()).gql_err("Invalid club ID")?;
        let row = infra::repos::clubs::get_by_id(&state.db, club_id).await?;
        Ok(row.map(Club::from))
    }
}

/// Manager input to add a person who is not (yet) an app user to the roster.
#[derive(InputObject)]
pub struct CreateRegisteredPlayerInput {
    pub club_id: ID,
    pub display_name: String,
}

/// Player input to claim an unclaimed roster entry as their own.
#[derive(InputObject)]
pub struct ClaimRegisteredPlayerInput {
    pub registered_player_id: ID,
}
