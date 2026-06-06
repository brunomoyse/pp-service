use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::features::{require_feature, Feature};
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::privacy;

use super::types::PrivacySettings;

fn current_user_id(ctx: &Context<'_>) -> Result<Uuid> {
    let claims = ctx.data::<Claims>()?;
    Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")
}

#[derive(Default)]
pub struct ScoutingQuery;

#[Object]
impl ScoutingQuery {
    /// The current user's privacy/consent settings (defaults all OFF).
    async fn my_privacy_settings(&self, ctx: &Context<'_>) -> Result<PrivacySettings> {
        require_feature(Feature::PublicStats)?;
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let row = privacy::get(&state.db, user_id).await?;
        Ok(row.map(PrivacySettings::from).unwrap_or_default())
    }
}

#[derive(Default)]
pub struct ScoutingMutation;

#[Object]
impl ScoutingMutation {
    /// Update the current user's consent flags. Both are granular and explicit
    /// (G4): the client must send each value, there is no implied bundling.
    async fn update_privacy_settings(
        &self,
        ctx: &Context<'_>,
        share_named_pl: bool,
        in_scouting_pool: bool,
    ) -> Result<PrivacySettings> {
        require_feature(Feature::PublicStats)?;
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let row = privacy::upsert(&state.db, user_id, share_named_pl, in_scouting_pool).await?;
        Ok(PrivacySettings::from(row))
    }
}
