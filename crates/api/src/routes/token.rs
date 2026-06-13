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

/// Header native clients use to present the raw refresh token, since they have
/// no cookie jar. Web clients keep using the HttpOnly `refresh_token` cookie.
const REFRESH_TOKEN_HEADER: &str = "x-refresh-token";

#[derive(Serialize)]
pub struct RefreshResponse {
    pub token: String,
    /// The rotated raw refresh token, returned **only** to native callers (those
    /// that authenticated via `X-Refresh-Token`) so they can persist the new
    /// token after rotation. Cookie callers get `None` and the Set-Cookie header.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

/// Pull the raw refresh token from the `X-Refresh-Token` header (native) or,
/// failing that, the `refresh_token` cookie (web). The bool is `true` when the
/// token came from the header, marking the caller as a native client.
fn extract_raw_refresh(req: &axum::extract::Request) -> Option<(String, bool)> {
    if let Some(header) = req
        .headers()
        .get(REFRESH_TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
    {
        let header = header.trim();
        if !header.is_empty() {
            return Some((header.to_string(), true));
        }
    }

    req.headers()
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(extract_refresh_token)
        .map(|token| (token, false))
}

pub async fn refresh_handler(
    State(state): State<AppState>,
    req: axum::extract::Request,
) -> Result<impl IntoResponse, AppError> {
    // Refresh token comes from the X-Refresh-Token header (native) or cookie (web)
    let (raw_token, is_native) = extract_raw_refresh(&req)
        .ok_or_else(|| AppError::Unauthorized("No refresh token provided".to_string()))?;

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
    // Preserve the login-time "remember me" choice across rotations: only a
    // remembered session gets a persistent (Max-Age) cookie back.
    let max_age_secs = if result.remember_me {
        Some(auth_config.refresh_token_expiration_days * 24 * 60 * 60)
    } else {
        None
    };
    let cookie_value = build_refresh_cookie(
        &result.new_raw_token,
        max_age_secs,
        &auth_config.cookie_domain,
        auth_config.cookie_secure,
    );

    // Native callers persist the rotated token from the body; web callers use
    // the Set-Cookie header below (which we still send harmlessly either way).
    let refresh_token = is_native.then(|| result.new_raw_token.clone());

    let mut response = Json(RefreshResponse {
        token,
        refresh_token,
    })
    .into_response();
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
    // Refresh token from header (native) or cookie (web) — optional, may already
    // be cleared. Revoke the entire token family when present.
    if let Some((raw_token, _)) = extract_raw_refresh(&req) {
        let _ = refresh::revoke_by_token(&state.db, &raw_token).await;
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
