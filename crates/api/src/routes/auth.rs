use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{OAuthProvider, custom_oauth::CustomOAuthService};
use crate::error::AppError;
use crate::gql::types::User;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct AuthorizeQuery {
    pub provider: String,
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: Option<String>,
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

    Ok(Json(AuthorizeResponse { auth_url, csrf_token }))
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

    // Generate JWT token
    let token = state
        .jwt_service()
        .create_token(user_id, oauth_user.email.clone())?;

    // Get user info for response
    let user = get_user_by_id(&state, user_id).await?;

    Ok(Json(AuthResponse { token, user }))
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
            role: row.role,
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
        role: row.role,
    })
}