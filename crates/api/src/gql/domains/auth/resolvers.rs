use async_graphql::{Context, Object, Result};
use rand::{distr::Alphanumeric, RngExt};
use uuid::Uuid;

use crate::auth::{
    custom_oauth::CustomOAuthService, password::PasswordService, Claims, OAuthProvider,
};
use crate::gql::error::ResultExt;
use crate::gql::types::User;
use crate::state::AppState;

use super::types::{
    AuthPayload, CreateOAuthClientInput, CreateOAuthClientResponse, OAuthCallbackInput,
    OAuthClient, OAuthUrlResponse, UserLoginInput, UserRegistrationInput,
};

// ── Queries ──────────────────────────────────────────────────────────

#[derive(Default)]
pub struct AuthQuery;

#[Object]
impl AuthQuery {
    /// Get the current authenticated user's information
    async fn me(&self, ctx: &Context<'_>) -> Result<User> {
        let claims = ctx
            .data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;

        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let state = ctx.data::<AppState>()?;

        let user = infra::repos::users::get_by_id(&state.db, user_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("User not found"))?;

        Ok(user.into())
    }

    /// Get OAuth authorization URL for a provider
    async fn get_oauth_url(&self, ctx: &Context<'_>, provider: String) -> Result<OAuthUrlResponse> {
        let state = ctx.data::<AppState>()?;

        let provider = match provider.as_str() {
            "google" => OAuthProvider::Google,
            "custom" => OAuthProvider::Custom,
            _ => return Err(async_graphql::Error::new("Invalid OAuth provider")),
        };

        let (auth_url, csrf_token) = state
            .oauth_service()
            .get_authorize_url(provider)
            .gql_err("Database operation failed")?;

        Ok(OAuthUrlResponse {
            auth_url,
            csrf_token,
        })
    }
}

// ── Mutations ────────────────────────────────────────────────────────

#[derive(Default)]
pub struct AuthMutation;

#[Object]
impl AuthMutation {
    /// Complete OAuth authentication flow
    async fn oauth_callback(
        &self,
        ctx: &Context<'_>,
        input: OAuthCallbackInput,
    ) -> Result<AuthPayload> {
        let state = ctx.data::<AppState>()?;

        let provider = match input.provider.as_str() {
            "google" => OAuthProvider::Google,
            "custom" => OAuthProvider::Custom,
            _ => return Err(async_graphql::Error::new("Invalid OAuth provider")),
        };

        let oauth_user = match provider {
            OAuthProvider::Custom => {
                CustomOAuthService::exchange_code_for_user_info(state, input.code)
                    .await
                    .gql_err("Database operation failed")?
            }
            _ => state
                .oauth_service()
                .exchange_code_for_user_info(provider, input.code)
                .await
                .gql_err("Database operation failed")?,
        };

        // Check if user exists, if not create them
        let user_id = match find_user_by_email(state, &oauth_user.email).await? {
            Some(existing_user) => {
                Uuid::parse_str(existing_user.id.as_str()).gql_err("Invalid user ID")?
            }
            None => create_user_from_oauth(state, &oauth_user, &input.provider).await?,
        };

        // Get user info for response
        let user = get_user_by_id(state, user_id).await?;

        // Generate JWT token
        let role_str: String = user.role.into();
        let token = state
            .jwt_service()
            .create_token(user_id, oauth_user.email.clone(), role_str)
            .gql_err("Database operation failed")?;

        // Create refresh token and set HttpOnly cookie
        let auth_config = state.auth_config();
        let raw_refresh = crate::auth::refresh::create_refresh_token(
            &state.db,
            user_id,
            auth_config.refresh_token_expiration_days,
        )
        .await
        .gql_err("Failed to create refresh token")?;

        let max_age_secs = auth_config.refresh_token_expiration_days * 24 * 60 * 60;
        let cookie_value = crate::auth::cookie::build_refresh_cookie(
            &raw_refresh,
            max_age_secs,
            &auth_config.cookie_domain,
        );
        ctx.insert_http_header("Set-Cookie", cookie_value);

        Ok(AuthPayload { token, user })
    }

    /// Validate token and get current user
    async fn me(&self, ctx: &Context<'_>) -> Result<User> {
        let claims = ctx.data::<Claims>().map_err(|_| {
            async_graphql::Error::new("You must be logged in to perform this action")
        })?;

        let state = ctx.data::<AppState>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        get_user_by_id(state, user_id).await
    }

    /// Create OAuth client (admin only)
    async fn create_oauth_client(
        &self,
        ctx: &Context<'_>,
        input: CreateOAuthClientInput,
    ) -> Result<CreateOAuthClientResponse> {
        use crate::auth::permissions::require_admin;
        let _admin = require_admin(ctx).await?;

        let state = ctx.data::<AppState>()?;

        let client_id = generate_client_id();
        let client_secret = generate_client_secret();

        let scopes = input.scopes.unwrap_or_else(|| vec!["read".to_string()]);

        let row = sqlx::query!(
            r#"
            INSERT INTO oauth_clients (client_id, client_secret, name, redirect_uris, scopes)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
            client_id,
            client_secret,
            input.name,
            &input.redirect_uris,
            &scopes
        )
        .fetch_one(&state.db)
        .await
        .gql_err("Database operation failed")?;

        let client = OAuthClient {
            id: row.id.into(),
            client_id: client_id.clone(),
            name: input.name,
            redirect_uris: input.redirect_uris,
            scopes,
            is_active: true,
        };

        Ok(CreateOAuthClientResponse {
            client,
            client_secret,
        })
    }

    /// Register user with password (for custom OAuth)
    async fn register_user(&self, ctx: &Context<'_>, input: UserRegistrationInput) -> Result<User> {
        let state = ctx.data::<AppState>()?;

        // Validate password strength
        PasswordService::validate_password_strength(&input.password)
            .gql_err("Database operation failed")?;

        // Hash password
        let password_hash =
            PasswordService::hash_password(&input.password).gql_err("Database operation failed")?;

        // Check if user already exists
        let existing_user = sqlx::query!("SELECT id FROM users WHERE email = $1", input.email)
            .fetch_optional(&state.db)
            .await
            .gql_err("Database operation failed")?;

        if existing_user.is_some() {
            return Err(async_graphql::Error::new(
                "User with this email already exists",
            ));
        }

        // Create user
        let row = sqlx::query!(
            r#"
            INSERT INTO users (email, first_name, last_name, username, password_hash)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
            input.email,
            input.first_name,
            input.last_name,
            input.username,
            password_hash
        )
        .fetch_one(&state.db)
        .await
        .gql_err("Database operation failed")?;

        Ok(User {
            id: row.id.into(),
            email: input.email,
            username: input.username,
            first_name: input.first_name,
            last_name: Some(input.last_name),
            phone: None,
            is_active: true,
            role: crate::gql::types::Role::Player,
        })
    }

    /// Login user with password (returns JWT token)
    async fn login_user(&self, ctx: &Context<'_>, input: UserLoginInput) -> Result<AuthPayload> {
        let state = ctx.data::<AppState>()?;

        // Find user by email
        let user_row = sqlx::query!(
            "SELECT id, email, username, first_name, last_name, phone, is_active, password_hash, role FROM users WHERE email = $1",
            input.email
        )
        .fetch_optional(&state.db)
        .await
        .gql_err("Database operation failed")?;

        let user_row = match user_row {
            Some(row) => row,
            None => return Err(async_graphql::Error::new("Invalid credentials")),
        };

        // Verify password
        if let Some(ref password_hash) = user_row.password_hash {
            if !PasswordService::verify_password(&input.password, password_hash)
                .gql_err("Database operation failed")?
            {
                return Err(async_graphql::Error::new("Invalid credentials"));
            }
        } else {
            return Err(async_graphql::Error::new("User has no password set"));
        }

        let role = crate::gql::types::Role::from(user_row.role);
        let role_str: String = role.into();

        // Generate JWT token
        let token = state
            .jwt_service()
            .create_token(user_row.id, user_row.email.clone(), role_str)
            .gql_err("Database operation failed")?;

        let user = User {
            id: user_row.id.into(),
            email: user_row.email,
            username: user_row.username,
            first_name: user_row.first_name,
            last_name: user_row.last_name,
            phone: user_row.phone,
            is_active: user_row.is_active,
            role,
        };

        // Create refresh token and set HttpOnly cookie
        let auth_config = state.auth_config();
        let raw_refresh = crate::auth::refresh::create_refresh_token(
            &state.db,
            user_row.id,
            auth_config.refresh_token_expiration_days,
        )
        .await
        .gql_err("Failed to create refresh token")?;

        let max_age_secs = auth_config.refresh_token_expiration_days * 24 * 60 * 60;
        let cookie_value = crate::auth::cookie::build_refresh_cookie(
            &raw_refresh,
            max_age_secs,
            &auth_config.cookie_domain,
        );
        ctx.insert_http_header("Set-Cookie", cookie_value);

        Ok(AuthPayload { token, user })
    }

    /// Get OAuth authorization URL for a provider
    async fn get_oauth_url(&self, ctx: &Context<'_>, provider: String) -> Result<OAuthUrlResponse> {
        let state = ctx.data::<AppState>()?;

        let provider = match provider.as_str() {
            "google" => OAuthProvider::Google,
            "custom" => OAuthProvider::Custom,
            _ => return Err(async_graphql::Error::new("Invalid OAuth provider")),
        };

        let (auth_url, csrf_token) = state
            .oauth_service()
            .get_authorize_url(provider)
            .gql_err("Database operation failed")?;

        Ok(OAuthUrlResponse {
            auth_url,
            csrf_token,
        })
    }
}

// ── Helper functions ─────────────────────────────────────────────────

fn generate_client_id() -> String {
    format!(
        "client_{}",
        rand::rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect::<String>()
    )
}

fn generate_client_secret() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

async fn find_user_by_email(state: &AppState, email: &str) -> Result<Option<User>> {
    let row = sqlx::query!(
        "SELECT id, email, username, first_name, last_name, phone, is_active, role FROM users WHERE email = $1",
        email
    )
    .fetch_optional(&state.db)
    .await
    .gql_err("Database operation failed")?;

    match row {
        Some(row) => Ok(Some(User {
            id: row.id.into(),
            email: row.email,
            username: row.username,
            first_name: row.first_name,
            last_name: row.last_name,
            phone: row.phone,
            is_active: row.is_active,
            role: crate::gql::types::Role::from(row.role),
        })),
        None => Ok(None),
    }
}

async fn create_user_from_oauth(
    state: &AppState,
    oauth_user: &crate::auth::oauth::OAuthUserInfo,
    provider: &str,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO users (email, username, first_name, last_name, oauth_provider, oauth_provider_id, avatar_url)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#,
        oauth_user.email,
        oauth_user.username,
        oauth_user.first_name,
        oauth_user.last_name,
        provider,
        oauth_user.provider_id,
        oauth_user.avatar_url
    )
    .fetch_one(&state.db)
    .await
    .gql_err("Database operation failed")?;

    Ok(row.id)
}

async fn get_user_by_id(state: &AppState, user_id: Uuid) -> Result<User> {
    let row = sqlx::query!(
        "SELECT id, email, username, first_name, last_name, phone, is_active, role FROM users WHERE id = $1",
        user_id
    )
    .fetch_one(&state.db)
    .await
    .gql_err("Database operation failed")?;

    Ok(User {
        id: row.id.into(),
        email: row.email,
        username: row.username,
        first_name: row.first_name,
        last_name: row.last_name,
        phone: row.phone,
        is_active: row.is_active,
        role: crate::gql::types::Role::from(row.role),
    })
}
