use chrono::{Duration, Utc};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthClient {
    pub id: Uuid,
    pub client_id: String,
    pub client_secret: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationCode {
    pub id: Uuid,
    pub code: String,
    pub client_id: Uuid,
    pub user_id: Uuid,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub challenge: Option<String>,
    pub challenge_method: Option<String>,
    pub expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessToken {
    pub id: Uuid,
    pub token: String,
    pub client_id: Uuid,
    pub user_id: Uuid,
    pub scopes: Vec<String>,
    pub expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshToken {
    pub id: Uuid,
    pub token: String,
    pub access_token_id: Uuid,
    pub client_id: Uuid,
    pub user_id: Uuid,
    pub expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AuthorizeRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: String,
    pub client_secret: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub error_description: Option<String>,
}

pub struct CustomOAuthService;

impl CustomOAuthService {
    pub async fn exchange_code_for_user_info(
        state: &AppState,
        code: String,
    ) -> Result<crate::auth::oauth::OAuthUserInfo, AppError> {
        // Get authorization code from database
        let auth_code = match Self::get_authorization_code(state, &code).await? {
            Some(auth_code) => auth_code,
            None => {
                return Err(AppError::BadRequest(
                    "Invalid or expired authorization code".to_string(),
                ))
            }
        };

        // Get user info from the database
        let user_row = sqlx::query!(
            "SELECT id, email, username, first_name, last_name FROM users WHERE id = $1",
            auth_code.user_id
        )
        .fetch_optional(&state.db)
        .await?;

        let user_row = match user_row {
            Some(row) => row,
            None => {
                return Err(AppError::Internal(
                    "User not found for authorization code".to_string(),
                ))
            }
        };

        // Delete used authorization code
        Self::delete_authorization_code(state, &code).await?;

        // Convert to OAuthUserInfo
        Ok(crate::auth::oauth::OAuthUserInfo {
            provider_id: user_row.id.to_string(),
            email: user_row.email,
            first_name: user_row.first_name,
            last_name: user_row.last_name,
            username: user_row.username,
            avatar_url: None,
        })
    }
    pub async fn get_client_by_id(
        state: &AppState,
        client_id: &str,
    ) -> Result<Option<OAuthClient>, AppError> {
        let row = sqlx::query!(
            "SELECT id, client_id, client_secret, name, redirect_uris, scopes, is_active FROM oauth_clients WHERE client_id = $1 AND is_active = true",
            client_id
        )
        .fetch_optional(&state.db)
        .await?;

        match row {
            Some(row) => Ok(Some(OAuthClient {
                id: row.id,
                client_id: row.client_id,
                client_secret: row.client_secret,
                name: row.name,
                redirect_uris: row.redirect_uris,
                scopes: row.scopes,
                is_active: row.is_active,
            })),
            None => Ok(None),
        }
    }

    pub async fn create_authorization_code(
        state: &AppState,
        client_id: Uuid,
        user_id: Uuid,
        redirect_uri: String,
        scopes: Vec<String>,
        challenge: Option<String>,
        challenge_method: Option<String>,
    ) -> Result<AuthorizationCode, AppError> {
        let code = Self::generate_code();
        let expires_at = Utc::now() + Duration::minutes(10); // 10-minute expiration

        let row = sqlx::query!(
            r#"
            INSERT INTO oauth_authorization_codes (code, client_id, user_id, redirect_uri, scopes, challenge, challenge_method, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id
            "#,
            code,
            client_id,
            user_id,
            redirect_uri,
            &scopes,
            challenge,
            challenge_method,
            expires_at
        )
        .fetch_one(&state.db)
        .await?;

        Ok(AuthorizationCode {
            id: row.id,
            code,
            client_id,
            user_id,
            redirect_uri,
            scopes,
            challenge,
            challenge_method,
            expires_at,
        })
    }

    pub async fn get_authorization_code(
        state: &AppState,
        code: &str,
    ) -> Result<Option<AuthorizationCode>, AppError> {
        let row = sqlx::query!(
            "SELECT id, code, client_id, user_id, redirect_uri, scopes, challenge, challenge_method, expires_at FROM oauth_authorization_codes WHERE code = $1",
            code
        )
        .fetch_optional(&state.db)
        .await?;

        match row {
            Some(row) => {
                // Check if code is expired
                if row.expires_at < Utc::now() {
                    Self::delete_authorization_code(state, code).await?;
                    return Ok(None);
                }

                Ok(Some(AuthorizationCode {
                    id: row.id,
                    code: row.code,
                    client_id: row.client_id,
                    user_id: row.user_id,
                    redirect_uri: row.redirect_uri,
                    scopes: row.scopes,
                    challenge: row.challenge,
                    challenge_method: row.challenge_method,
                    expires_at: row.expires_at,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn delete_authorization_code(state: &AppState, code: &str) -> Result<(), AppError> {
        sqlx::query!(
            "DELETE FROM oauth_authorization_codes WHERE code = $1",
            code
        )
        .execute(&state.db)
        .await?;
        Ok(())
    }

    pub async fn create_access_token(
        state: &AppState,
        client_id: Uuid,
        user_id: Uuid,
        scopes: Vec<String>,
    ) -> Result<(AccessToken, RefreshToken), AppError> {
        let access_token = Self::generate_token();
        let refresh_token_str = Self::generate_token();
        let access_expires_at = Utc::now() + Duration::hours(1);
        let refresh_expires_at = Utc::now() + Duration::days(30);

        // Create access token
        let access_row = sqlx::query!(
            r#"
            INSERT INTO oauth_access_tokens (token, client_id, user_id, scopes, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
            access_token,
            client_id,
            user_id,
            &scopes,
            access_expires_at
        )
        .fetch_one(&state.db)
        .await?;

        // Create refresh token
        let refresh_row = sqlx::query!(
            r#"
            INSERT INTO oauth_refresh_tokens (token, access_token_id, client_id, user_id, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
            refresh_token_str,
            access_row.id,
            client_id,
            user_id,
            refresh_expires_at
        )
        .fetch_one(&state.db)
        .await?;

        let access_token_obj = AccessToken {
            id: access_row.id,
            token: access_token,
            client_id,
            user_id,
            scopes: scopes.clone(),
            expires_at: access_expires_at,
        };

        let refresh_token_obj = RefreshToken {
            id: refresh_row.id,
            token: refresh_token_str,
            access_token_id: access_row.id,
            client_id,
            user_id,
            expires_at: refresh_expires_at,
        };

        Ok((access_token_obj, refresh_token_obj))
    }

    pub async fn validate_redirect_uri(
        client: &OAuthClient,
        redirect_uri: &str,
    ) -> Result<bool, AppError> {
        Ok(client.redirect_uris.contains(&redirect_uri.to_string()))
    }

    pub fn parse_scopes(scope_string: Option<String>) -> Vec<String> {
        scope_string
            .unwrap_or_else(|| "read".to_string())
            .split_whitespace()
            .map(|s| s.to_string())
            .collect()
    }

    pub fn validate_scopes(requested_scopes: &[String], client_scopes: &[String]) -> bool {
        requested_scopes
            .iter()
            .all(|scope| client_scopes.contains(scope))
    }

    fn generate_code() -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect()
    }

    fn generate_token() -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect()
    }
}
