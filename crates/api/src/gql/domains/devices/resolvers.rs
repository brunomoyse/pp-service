use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::device_tokens;

use super::types::RegisterDeviceTokenInput;

#[derive(Default)]
pub struct DeviceMutation;

#[Object]
impl DeviceMutation {
    /// Register (or refresh) this device's Expo push token for the current user
    /// so the server can deliver push notifications to it.
    async fn register_device_token(
        &self,
        ctx: &Context<'_>,
        input: RegisterDeviceTokenInput,
    ) -> Result<bool> {
        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        device_tokens::upsert(&state.db, user_id, &input.token, input.platform.as_str()).await?;
        Ok(true)
    }

    /// Drop this device's push token on logout so it stops receiving the
    /// current user's notifications.
    async fn unregister_device_token(&self, ctx: &Context<'_>, token: String) -> Result<bool> {
        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        device_tokens::delete_for_user(&state.db, user_id, &token).await?;
        Ok(true)
    }
}
