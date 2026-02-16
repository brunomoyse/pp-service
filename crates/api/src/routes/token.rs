use axum::{
    extract::State,
    http::header::{COOKIE, SET_COOKIE},
    response::IntoResponse,
    Json,
};
use serde::Serialize;

use crate::auth::cookie::{build_clear_cookie, build_refresh_cookie, extract_refresh_token};
use crate::auth::refresh;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Serialize)]
pub struct RefreshResponse {
    pub token: String,
}

pub async fn refresh_handler(
    State(state): State<AppState>,
    req: axum::extract::Request,
) -> Result<impl IntoResponse, AppError> {
    // Extract refresh token from Cookie header
    let cookie_header = req
        .headers()
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("No cookie provided".to_string()))?;

    let raw_token = extract_refresh_token(cookie_header)
        .ok_or_else(|| AppError::Unauthorized("No refresh token in cookie".to_string()))?;

    // Rotate the refresh token
    let result = refresh::rotate_refresh_token(
        &state.db,
        &raw_token,
        state.auth_config().refresh_token_expiration_days,
    )
    .await?;

    // Look up the user to generate a new JWT
    let user_row = infra::repos::users::get_by_id(&state.db, result.user_id)
        .await
        .map_err(|e| AppError::Internal(format!("DB error: {}", e)))?
        .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

    let role_str: String = crate::gql::types::Role::from(user_row.role).into();
    let token = state
        .jwt_service()
        .create_token(result.user_id, user_row.email, role_str)?;

    // Build new refresh cookie
    let auth_config = state.auth_config();
    let max_age_secs = auth_config.refresh_token_expiration_days * 24 * 60 * 60;
    let cookie_value = build_refresh_cookie(
        &result.new_raw_token,
        max_age_secs,
        &auth_config.cookie_domain,
        auth_config.cookie_secure,
    );

    let mut response = Json(RefreshResponse { token }).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        cookie_value
            .parse()
            .map_err(|_| AppError::Internal("Failed to build cookie header".to_string()))?,
    );

    Ok(response)
}

pub async fn logout_handler(
    State(state): State<AppState>,
    req: axum::extract::Request,
) -> Result<impl IntoResponse, AppError> {
    // Extract refresh token from Cookie header (optional — may already be cleared)
    if let Some(cookie_header) = req.headers().get(COOKIE).and_then(|v| v.to_str().ok()) {
        if let Some(raw_token) = extract_refresh_token(cookie_header) {
            // Revoke the entire token family
            let _ = refresh::revoke_by_token(&state.db, &raw_token).await;
        }
    }

    // Clear the cookie
    let auth_config = state.auth_config();
    let cookie_value = build_clear_cookie(&auth_config.cookie_domain, auth_config.cookie_secure);

    let mut response = axum::http::StatusCode::OK.into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        cookie_value
            .parse()
            .map_err(|_| AppError::Internal("Failed to build cookie header".to_string()))?,
    );

    Ok(response)
}
