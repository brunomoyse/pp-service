use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::auth::permissions::require_club_manager;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::registered_players;

use super::service;
use super::types::{ClaimRegisteredPlayerInput, CreateRegisteredPlayerInput, RegisteredPlayer};

#[derive(Default)]
pub struct IdentityQuery;

#[Object]
impl IdentityQuery {
    /// Full club roster (app users and non-users alike). Managers of the club only.
    async fn registered_players(
        &self,
        ctx: &Context<'_>,
        club_id: ID,
    ) -> Result<Vec<RegisteredPlayer>> {
        let club_uuid = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_uuid).await?;

        let state = ctx.data::<AppState>()?;
        let rows = registered_players::list_by_club(&state.db, club_uuid).await?;
        Ok(rows.into_iter().map(RegisteredPlayer::from).collect())
    }

    /// The current user's roster entries across every club — the cross-club profile.
    async fn my_cross_club_profile(&self, ctx: &Context<'_>) -> Result<Vec<RegisteredPlayer>> {
        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let rows = registered_players::list_for_app_user(&state.db, user_id).await?;
        Ok(rows.into_iter().map(RegisteredPlayer::from).collect())
    }
}

#[derive(Default)]
pub struct IdentityMutation;

#[Object]
impl IdentityMutation {
    /// Add someone the club registers who is not (yet) an app user to the roster.
    /// Managers of the club only.
    async fn create_registered_player(
        &self,
        ctx: &Context<'_>,
        input: CreateRegisteredPlayerInput,
    ) -> Result<RegisteredPlayer> {
        let club_uuid = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_uuid).await?;

        let state = ctx.data::<AppState>()?;
        let row = service::create_roster_entry(&state.db, club_uuid, &input.display_name).await?;
        Ok(RegisteredPlayer::from(row))
    }

    /// Claim an unclaimed roster entry, linking it to the current app user.
    async fn claim_registered_player(
        &self,
        ctx: &Context<'_>,
        input: ClaimRegisteredPlayerInput,
    ) -> Result<RegisteredPlayer> {
        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;
        let rp_id = Uuid::parse_str(input.registered_player_id.as_str())
            .gql_err("Invalid registered player ID")?;

        let row = service::claim_roster_entry(&state.db, rp_id, user_id).await?;
        Ok(RegisteredPlayer::from(row))
    }
}
