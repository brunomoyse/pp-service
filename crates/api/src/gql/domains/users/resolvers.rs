use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::gql::error::ResultExt;
use crate::gql::types::{PaginatedResponse, PaginationInput, Role, User};
use crate::state::AppState;
use infra::repos::{
    users,
    users::{CreateUserData, UpdateUserData, UserFilter},
};

use super::types::{
    CreatePlayerInput, NotificationPreferences, UpdateNotificationPreferencesInput,
    UpdatePlayerInput,
};

#[derive(Default)]
pub struct UserQuery;

#[Object]
impl UserQuery {
    async fn users(
        &self,
        ctx: &Context<'_>,
        search: Option<String>,
        is_active: Option<bool>,
        pagination: Option<PaginationInput>,
    ) -> Result<PaginatedResponse<User>> {
        let state = ctx.data::<AppState>()?;
        let filter = UserFilter { search, is_active };

        let page_params = pagination.unwrap_or(PaginationInput {
            limit: Some(50),
            offset: Some(0),
        });
        let limit_offset = page_params.to_limit_offset();

        // Fetch users and total count in parallel
        let (rows, total_count) = tokio::try_join!(
            users::list(&state.db, filter.clone(), Some(limit_offset)),
            users::count(&state.db, filter)
        )?;

        let items: Vec<User> = rows.into_iter().map(User::from).collect();
        let page_size = items.len() as i32;
        let offset = limit_offset.offset as i32;
        let has_next_page = (offset + page_size) < total_count as i32;

        Ok(PaginatedResponse {
            items,
            total_count: total_count as i32,
            page_size,
            offset,
            has_next_page,
        })
    }

    /// The current user's notification preferences (defaults when never set).
    async fn my_notification_preferences(
        &self,
        ctx: &Context<'_>,
    ) -> Result<NotificationPreferences> {
        use crate::auth::jwt::Claims;

        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let prefs =
            infra::repos::notification_preferences::get_for_user(&state.db, user_id).await?;
        Ok(prefs.into())
    }
}

#[derive(Default)]
pub struct UserMutation;

#[Object]
impl UserMutation {
    async fn create_player(&self, ctx: &Context<'_>, input: CreatePlayerInput) -> Result<User> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;

        // Parse club_id and require club manager permissions
        let club_id = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        let _manager = require_club_manager(ctx, club_id).await?;

        // Check if user with email already exists
        let existing = sqlx::query!("SELECT id FROM users WHERE email = $1", input.email)
            .fetch_optional(&state.db)
            .await
            .gql_err("Database operation failed")?;

        if existing.is_some() {
            return Err(async_graphql::Error::new(
                "A user with this email already exists",
            ));
        }

        let create_data = CreateUserData {
            email: input.email,
            first_name: input.first_name,
            last_name: input.last_name,
            username: input.username,
            phone: input.phone,
        };

        let user_row = users::create(&state.db, create_data).await?;

        Ok(user_row.into())
    }

    /// Update an existing player (managers only)
    async fn update_player(&self, ctx: &Context<'_>, input: UpdatePlayerInput) -> Result<User> {
        use crate::auth::permissions::require_role;

        // TODO: users aren't club-scoped, keep role-based for now
        let _manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;

        let user_id = Uuid::parse_str(input.id.as_str()).gql_err("Invalid user ID")?;

        // Check if user exists
        let existing = users::get_by_id(&state.db, user_id).await?;
        if existing.is_none() {
            return Err(async_graphql::Error::new("User not found"));
        }

        // If email is being updated, check it's not taken by another user
        if let Some(ref new_email) = input.email {
            let email_taken = sqlx::query!(
                "SELECT id FROM users WHERE email = $1 AND id != $2",
                new_email,
                user_id
            )
            .fetch_optional(&state.db)
            .await
            .gql_err("Database operation failed")?;

            if email_taken.is_some() {
                return Err(async_graphql::Error::new(
                    "A user with this email already exists",
                ));
            }
        }

        let update_data = UpdateUserData {
            email: input.email,
            first_name: input.first_name,
            last_name: input.last_name,
            username: input.username,
            phone: input.phone,
        };

        let user_row = users::update(&state.db, user_id, update_data)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Failed to update user"))?;

        Ok(user_row.into())
    }

    /// Deactivate a player (soft delete) - managers only
    async fn deactivate_player(&self, ctx: &Context<'_>, id: ID) -> Result<User> {
        use crate::auth::permissions::require_role;

        // TODO: users aren't club-scoped, keep role-based for now
        let _manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;

        let user_id = Uuid::parse_str(id.as_str()).gql_err("Invalid user ID")?;

        let user_row = users::deactivate(&state.db, user_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("User not found"))?;

        Ok(user_row.into())
    }

    /// Update the current user's notification preferences. Omitted fields
    /// keep their current value.
    async fn update_notification_preferences(
        &self,
        ctx: &Context<'_>,
        input: UpdateNotificationPreferencesInput,
    ) -> Result<NotificationPreferences> {
        use crate::auth::jwt::Claims;
        use infra::repos::notification_preferences;

        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let mut prefs = notification_preferences::get_for_user(&state.db, user_id).await?;
        if let Some(v) = input.tournament_reminders {
            prefs.tournament_reminders = v;
        }
        if let Some(v) = input.registration_updates {
            prefs.registration_updates = v;
        }
        if let Some(v) = input.seating_updates {
            prefs.seating_updates = v;
        }
        if let Some(v) = input.achievements {
            prefs.achievements = v;
        }
        notification_preferences::upsert(&state.db, user_id, prefs).await?;

        Ok(prefs.into())
    }

    /// Permanently delete the current user's account (self-service, required
    /// for App Store / GDPR). Personal data is anonymized and the account is
    /// deactivated; tournament history is kept under an anonymous name.
    async fn delete_my_account(&self, ctx: &Context<'_>) -> Result<bool> {
        use crate::auth::jwt::Claims;
        use infra::repos::{device_tokens, refresh_tokens};

        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        // Scrub PII and deactivate first — that's the part that must not be
        // missed; session/device cleanup below is best-effort on top.
        users::anonymize(&state.db, user_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("User not found"))?;

        device_tokens::delete_all_for_user(&state.db, user_id).await?;
        refresh_tokens::revoke_all_for_user(&state.db, user_id).await?;

        Ok(true)
    }

    /// Reactivate a player - managers only
    async fn reactivate_player(&self, ctx: &Context<'_>, id: ID) -> Result<User> {
        use crate::auth::permissions::require_role;

        // TODO: users aren't club-scoped, keep role-based for now
        let _manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;

        let user_id = Uuid::parse_str(id.as_str()).gql_err("Invalid user ID")?;

        let user_row = users::reactivate(&state.db, user_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("User not found"))?;

        Ok(user_row.into())
    }
}
