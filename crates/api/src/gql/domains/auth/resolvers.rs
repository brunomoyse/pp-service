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
    OAuthClient, OAuthUrlResponse, RequestPasswordResetInput, RequestPasswordResetResponse,
    ResetPasswordInput, ResetPasswordResponse, UserLoginInput, UserRegistrationInput,
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

        let max_age_secs = Some(auth_config.refresh_token_expiration_days * 24 * 60 * 60);
        let cookie_value = crate::auth::cookie::build_refresh_cookie(
            &raw_refresh,
            max_age_secs,
            &auth_config.cookie_domain,
            auth_config.cookie_secure,
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
            locale: "en".to_string(),
        })
    }

    /// Login user with password (returns JWT token)
    async fn login_user(&self, ctx: &Context<'_>, input: UserLoginInput) -> Result<AuthPayload> {
        use sqlx::Row;

        let state = ctx.data::<AppState>()?;

        // Find user by email — only fetch id + password_hash for auth check
        let auth_row = sqlx::query("SELECT id, password_hash FROM users WHERE email = $1")
            .bind(&input.email)
            .fetch_optional(&state.db)
            .await
            .gql_err("Database operation failed")?;

        let auth_row = match auth_row {
            Some(row) => row,
            None => return Err(async_graphql::Error::new("Invalid credentials")),
        };

        let user_id: Uuid = auth_row.get("id");
        let password_hash: Option<String> = auth_row.get("password_hash");

        // Verify password
        if let Some(ref hash) = password_hash {
            if !PasswordService::verify_password(&input.password, hash)
                .gql_err("Database operation failed")?
            {
                return Err(async_graphql::Error::new("Invalid credentials"));
            }
        } else {
            return Err(async_graphql::Error::new("User has no password set"));
        }

        // Fetch full user data via repo (includes locale)
        let user: User = infra::repos::users::get_by_id(&state.db, user_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("User not found"))?
            .into();

        let role_str: String = user.role.into();

        // Generate JWT token
        let token = state
            .jwt_service()
            .create_token(user_id, user.email.clone(), role_str)
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

        // "Remember me" → persistent cookie with Max-Age; otherwise session cookie
        let max_age_secs = if input.remember_me {
            Some(auth_config.refresh_token_expiration_days * 24 * 60 * 60)
        } else {
            None
        };
        let cookie_value = crate::auth::cookie::build_refresh_cookie(
            &raw_refresh,
            max_age_secs,
            &auth_config.cookie_domain,
            auth_config.cookie_secure,
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

    /// Request a password reset email (unauthenticated)
    async fn request_password_reset(
        &self,
        ctx: &Context<'_>,
        input: RequestPasswordResetInput,
    ) -> Result<RequestPasswordResetResponse> {
        use chrono::{Duration, Utc};
        use rand::RngExt;

        let state = ctx.data::<AppState>()?;

        // Look up user by email
        let user = find_user_by_email(state, &input.email).await?;

        if let Some(user) = user {
            let user_id = Uuid::parse_str(user.id.as_str()).gql_err("Invalid user ID")?;

            // Invalidate any existing pending tokens for this user
            infra::repos::password_reset_tokens::invalidate_for_user(&state.db, user_id)
                .await
                .gql_err("Database operation failed")?;

            // Generate a 64-char random token
            let raw_token: String = rand::rng()
                .sample_iter(&Alphanumeric)
                .take(64)
                .map(char::from)
                .collect();

            // Hash and store
            let token_hash = crate::auth::refresh::hash_token(&raw_token);
            let expires_at = Utc::now() + Duration::hours(1);

            infra::repos::password_reset_tokens::create(
                &state.db,
                &token_hash,
                user_id,
                expires_at,
            )
            .await
            .gql_err("Database operation failed")?;

            // Send email (awaited, not fire-and-forget)
            if let Some(email_service) = state.email_service() {
                let display_name = user.first_name.clone();
                let locale = crate::services::email_service::Locale::from_str_lossy(
                    input.locale.as_deref().unwrap_or(&user.locale),
                );
                if let Err(e) = email_service
                    .send_password_reset(&user.email, &display_name, &raw_token, locale)
                    .await
                {
                    tracing::error!("Failed to send password reset email: {}", e);
                }
            }
        }

        // Always return success to prevent user enumeration
        Ok(RequestPasswordResetResponse {
            success: true,
            message: "If an account with that email exists, a password reset link has been sent."
                .to_string(),
        })
    }

    /// Reset password using a token (unauthenticated)
    async fn reset_password(
        &self,
        ctx: &Context<'_>,
        input: ResetPasswordInput,
    ) -> Result<ResetPasswordResponse> {
        let state = ctx.data::<AppState>()?;

        // Validate password strength
        PasswordService::validate_password_strength(&input.new_password)
            .gql_err("Password validation failed")?;

        // Hash the incoming token and look up
        let token_hash = crate::auth::refresh::hash_token(&input.token);
        let token_row =
            infra::repos::password_reset_tokens::find_valid_by_hash(&state.db, &token_hash)
                .await
                .gql_err("Database operation failed")?;

        let token_row = match token_row {
            Some(row) => row,
            None => {
                return Ok(ResetPasswordResponse {
                    success: false,
                    message: "Invalid or expired reset token.".to_string(),
                });
            }
        };

        // Hash new password
        let password_hash = PasswordService::hash_password(&input.new_password)
            .gql_err("Password hashing failed")?;

        // Update user password
        sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(&password_hash)
            .bind(token_row.user_id)
            .execute(&state.db)
            .await
            .gql_err("Database operation failed")?;

        // Mark token as used
        infra::repos::password_reset_tokens::mark_used(&state.db, token_row.id)
            .await
            .gql_err("Database operation failed")?;

        Ok(ResetPasswordResponse {
            success: true,
            message: "Password has been reset successfully.".to_string(),
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
    let row = sqlx::query_as::<_, infra::models::UserRow>(
        "SELECT id, email, username, first_name, last_name, phone, is_active, role, locale, created_at, updated_at FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(&state.db)
    .await
    .gql_err("Database operation failed")?;

    Ok(row.map(User::from))
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
    let row = infra::repos::users::get_by_id(&state.db, user_id)
        .await
        .gql_err("Database operation failed")?
        .ok_or_else(|| async_graphql::Error::new("User not found"))?;

    Ok(User::from(row))
}
