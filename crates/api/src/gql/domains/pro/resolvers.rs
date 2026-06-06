use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::auth::permissions::{require_admin, require_club_manager};
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::pro_entitlements;

use super::types::{GrantProEntitlementInput, ProEntitlement};

#[derive(Default)]
pub struct ProQuery;

#[Object]
impl ProQuery {
    /// The current user's Pro entitlements (active + history).
    async fn my_pro_entitlements(&self, ctx: &Context<'_>) -> Result<Vec<ProEntitlement>> {
        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let rows = pro_entitlements::list_for_user(&state.db, user_id).await?;
        Ok(rows.into_iter().map(ProEntitlement::from).collect())
    }
}

#[derive(Default)]
pub struct ProMutation;

#[Object]
impl ProMutation {
    /// Gift Pro to a user on behalf of a club. Managers of that club only.
    async fn grant_pro_entitlement(
        &self,
        ctx: &Context<'_>,
        input: GrantProEntitlementInput,
    ) -> Result<ProEntitlement> {
        let club_id = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        let manager = require_club_manager(ctx, club_id).await?;

        let app_user_id = Uuid::parse_str(input.app_user_id.as_str()).gql_err("Invalid user ID")?;
        let granted_by = Uuid::parse_str(manager.id.as_str()).gql_err("Invalid user ID")?;

        let state = ctx.data::<AppState>()?;
        let row = pro_entitlements::grant(
            &state.db,
            app_user_id,
            "club_gift",
            Some(club_id),
            Some(granted_by),
            input.expires_at,
            input.notes.as_deref(),
        )
        .await?;
        Ok(ProEntitlement::from(row))
    }

    /// Revoke an entitlement. The granting club's manager (or an admin) only.
    async fn revoke_pro_entitlement(&self, ctx: &Context<'_>, entitlement_id: ID) -> Result<bool> {
        let id = Uuid::parse_str(entitlement_id.as_str()).gql_err("Invalid entitlement ID")?;
        let state = ctx.data::<AppState>()?;

        let row = pro_entitlements::get_by_id(&state.db, id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Entitlement not found"))?;

        match row.granted_by_club_id {
            Some(club_id) => {
                require_club_manager(ctx, club_id).await?;
            }
            None => {
                require_admin(ctx).await?;
            }
        }

        Ok(pro_entitlements::revoke(&state.db, id).await?.is_some())
    }
}
