use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::gql::error::ResultExt;
use crate::gql::types::{Role, User};
use crate::state::AppState;
use infra::{
    pagination::LimitOffset,
    repos::{
        users,
        users::{CreateUserData, UpdateUserData, UserFilter},
    },
};

use super::types::{CreatePlayerInput, UpdatePlayerInput};

#[derive(Default)]
pub struct UserQuery;

#[Object]
impl UserQuery {
    async fn users(
        &self,
        ctx: &Context<'_>,
        search: Option<String>,
        is_active: Option<bool>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<User>> {
        let state = ctx.data::<AppState>()?;
        let filter = UserFilter { search, is_active };
        let page = Some(LimitOffset {
            limit: limit.unwrap_or(50).clamp(1, 200),
            offset: offset.unwrap_or(0).max(0),
        });
        let rows = users::list(&state.db, filter, page).await?;
        Ok(rows
            .into_iter()
            .map(|r| User {
                id: r.id.into(),
                email: r.email,
                username: r.username,
                first_name: r.first_name,
                last_name: r.last_name,
                phone: r.phone,
                is_active: r.is_active,
                role: Role::from(r.role),
            })
            .collect())
    }
}

#[derive(Default)]
pub struct UserMutation;

#[Object]
impl UserMutation {
    async fn create_player(&self, ctx: &Context<'_>, input: CreatePlayerInput) -> Result<User> {
        use crate::auth::permissions::require_role;

        // Require manager role
        let _manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;

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

        Ok(User {
            id: user_row.id.into(),
            email: user_row.email,
            username: user_row.username,
            first_name: user_row.first_name,
            last_name: user_row.last_name,
            phone: user_row.phone,
            is_active: user_row.is_active,
            role: Role::from(user_row.role),
        })
    }

    /// Update an existing player (managers only)
    async fn update_player(&self, ctx: &Context<'_>, input: UpdatePlayerInput) -> Result<User> {
        use crate::auth::permissions::require_role;

        // Require manager role
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

        Ok(User {
            id: user_row.id.into(),
            email: user_row.email,
            username: user_row.username,
            first_name: user_row.first_name,
            last_name: user_row.last_name,
            phone: user_row.phone,
            is_active: user_row.is_active,
            role: Role::from(user_row.role),
        })
    }

    /// Deactivate a player (soft delete) - managers only
    async fn deactivate_player(&self, ctx: &Context<'_>, id: ID) -> Result<User> {
        use crate::auth::permissions::require_role;

        // Require manager role
        let _manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;

        let user_id = Uuid::parse_str(id.as_str()).gql_err("Invalid user ID")?;

        let user_row = users::deactivate(&state.db, user_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("User not found"))?;

        Ok(User {
            id: user_row.id.into(),
            email: user_row.email,
            username: user_row.username,
            first_name: user_row.first_name,
            last_name: user_row.last_name,
            phone: user_row.phone,
            is_active: user_row.is_active,
            role: Role::from(user_row.role),
        })
    }

    /// Reactivate a player - managers only
    async fn reactivate_player(&self, ctx: &Context<'_>, id: ID) -> Result<User> {
        use crate::auth::permissions::require_role;

        // Require manager role
        let _manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;

        let user_id = Uuid::parse_str(id.as_str()).gql_err("Invalid user ID")?;

        let user_row = users::reactivate(&state.db, user_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("User not found"))?;

        Ok(User {
            id: user_row.id.into(),
            email: user_row.email,
            username: user_row.username,
            first_name: user_row.first_name,
            last_name: user_row.last_name,
            phone: user_row.phone,
            is_active: user_row.is_active,
            role: Role::from(user_row.role),
        })
    }
}
