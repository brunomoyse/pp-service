use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::achievements;

use super::types::{Achievement, PlayerAchievement};

#[derive(Default)]
pub struct AchievementQuery;

#[Object]
impl AchievementQuery {
    /// Get the catalog of all available achievements
    async fn achievements(&self, ctx: &Context<'_>) -> Result<Vec<Achievement>> {
        let state = ctx.data::<AppState>()?;
        let rows = achievements::list_catalog(&state.db).await?;
        Ok(rows.into_iter().map(Achievement::from).collect())
    }

    /// Get achievements for the current user with their progress
    async fn my_achievements(&self, ctx: &Context<'_>) -> Result<Vec<PlayerAchievement>> {
        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;

        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let rows = achievements::list_for_player(&state.db, user_id).await?;
        Ok(rows.into_iter().map(PlayerAchievement::from).collect())
    }
}
