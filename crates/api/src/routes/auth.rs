use axum::{
    extract::{Path, Query, State},
    http::header::SET_COOKIE,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{cookie::build_refresh_cookie, custom_oauth::CustomOAuthService, OAuthProvider};
use crate::error::AppError;
use crate::gql::types::User;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
}

#[derive(Serialize)]
pub struct AuthorizeResponse {
    pub auth_url: String,
    pub csrf_token: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

pub async fn authorize(
    State(state): State<AppState>,
    Path(provider_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let provider = match provider_str.as_str() {
        "google" => OAuthProvider::Google,
        "custom" => OAuthProvider::Custom,
        _ => return Err(AppError::BadRequest("Invalid OAuth provider".to_string())),
    };

    let (auth_url, csrf_token) = state.oauth_service().get_authorize_url(provider)?;

    Ok(Json(AuthorizeResponse {
        auth_url,
        csrf_token,
    }))
}

pub async fn callback(
    State(state): State<AppState>,
    Path(provider_str): Path<String>,
    Query(query): Query<CallbackQuery>,
) -> Result<impl IntoResponse, AppError> {
    let provider = match provider_str.as_str() {
        "google" => OAuthProvider::Google,
        "custom" => OAuthProvider::Custom,
        _ => return Err(AppError::BadRequest("Invalid OAuth provider".to_string())),
    };

    let oauth_user = match provider {
        OAuthProvider::Custom => {
            CustomOAuthService::exchange_code_for_user_info(&state, query.code).await?
        }
        _ => {
            state
                .oauth_service()
                .exchange_code_for_user_info(provider, query.code)
                .await?
        }
    };

    // Check if user exists, if not create them
    let user_id = match find_user_by_email(&state, &oauth_user.email).await? {
        Some(existing_user) => Uuid::parse_str(existing_user.id.as_str())
            .map_err(|e| AppError::Internal(format!("Invalid user ID: {}", e)))?,
        None => create_user_from_oauth(&state, oauth_user.clone(), &provider_str).await?,
    };

    // Get user info for response
    let user = get_user_by_id(&state, user_id).await?;

    // Generate JWT token
    let role_str: String = user.role.into();
    let token = state
        .jwt_service()
        .create_token(user_id, oauth_user.email.clone(), role_str)?;

    // Create refresh token and set HttpOnly cookie
    let auth_config = state.auth_config();
    let raw_refresh = crate::auth::refresh::create_refresh_token(
        &state.db,
        user_id,
        auth_config.refresh_token_expiration_days,
    )
    .await?;

    let max_age_secs = auth_config.refresh_token_expiration_days * 24 * 60 * 60;
    let cookie_value = build_refresh_cookie(&raw_refresh, max_age_secs, &auth_config.cookie_domain);

    let mut response = Json(AuthResponse { token, user }).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        cookie_value
            .parse()
            .map_err(|_| AppError::Internal("Failed to build cookie header".to_string()))?,
    );

    Ok(response)
}

async fn find_user_by_email(state: &AppState, email: &str) -> Result<Option<User>, AppError> {
    let row = sqlx::query!(
        "SELECT id, email, username, first_name, last_name, phone, is_active, role FROM users WHERE email = $1",
        email
    )
    .fetch_optional(&state.db)
    .await?;

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
    oauth_user: crate::auth::oauth::OAuthUserInfo,
    provider: &str,
) -> Result<Uuid, AppError> {
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
    .await?;

    Ok(row.id)
}

async fn get_user_by_id(state: &AppState, user_id: Uuid) -> Result<User, AppError> {
    let row = sqlx::query!(
        "SELECT id, email, username, first_name, last_name, phone, is_active, role FROM users WHERE id = $1",
        user_id
    )
    .fetch_one(&state.db)
    .await?;

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
