use chrono::{Duration, Utc};
use rand::distr::Alphanumeric;
use rand::RngExt;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

pub struct RotateResult {
    pub user_id: Uuid,
    pub new_raw_token: String,
}

pub fn hash_token(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub async fn create_refresh_token(
    pool: &PgPool,
    user_id: Uuid,
    expiration_days: u64,
) -> Result<String, AppError> {
    let raw_token: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();

    let token_hash = hash_token(&raw_token);
    let family_id = Uuid::new_v4();
    let expires_at = Utc::now() + Duration::days(expiration_days as i64);

    infra::repos::refresh_tokens::create(pool, &token_hash, user_id, family_id, expires_at)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create refresh token: {}", e)))?;

    Ok(raw_token)
}

pub async fn rotate_refresh_token(
    pool: &PgPool,
    raw_token: &str,
    expiration_days: u64,
) -> Result<RotateResult, AppError> {
    let token_hash = hash_token(raw_token);

    // Check if the token exists and is active
    let existing = infra::repos::refresh_tokens::find_by_hash(pool, &token_hash)
        .await
        .map_err(|e| AppError::Internal(format!("DB error: {}", e)))?;

    match existing {
        Some(token_row) => {
            // Revoke the old token
            infra::repos::refresh_tokens::revoke(pool, token_row.id)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to revoke old token: {}", e)))?;

            // Create new token in the same family
            let new_raw: String = rand::rng()
                .sample_iter(&Alphanumeric)
                .take(64)
                .map(char::from)
                .collect();

            let new_hash = hash_token(&new_raw);
            let expires_at = Utc::now() + Duration::days(expiration_days as i64);

            infra::repos::refresh_tokens::create(
                pool,
                &new_hash,
                token_row.user_id,
                token_row.family_id,
                expires_at,
            )
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create new token: {}", e)))?;

            Ok(RotateResult {
                user_id: token_row.user_id,
                new_raw_token: new_raw,
            })
        }
        None => {
            // Token not found as active — check if it was revoked (theft detection)
            let was_revoked = infra::repos::refresh_tokens::is_revoked(pool, &token_hash)
                .await
                .map_err(|e| AppError::Internal(format!("DB error: {}", e)))?;

            if was_revoked {
                // Theft detected! Find the family and revoke everything.
                let family_id =
                    infra::repos::refresh_tokens::find_family_id_by_hash(pool, &token_hash)
                        .await
                        .map_err(|e| AppError::Internal(format!("DB error: {}", e)))?;

                if let Some(family_id) = family_id {
                    infra::repos::refresh_tokens::revoke_family(pool, family_id)
                        .await
                        .map_err(|e| {
                            AppError::Internal(format!("Failed to revoke family: {}", e))
                        })?;
                    tracing::warn!(
                        "Refresh token theft detected! Revoked token family {}",
                        family_id
                    );
                }
            }

            Err(AppError::Unauthorized(
                "Invalid or expired refresh token".to_string(),
            ))
        }
    }
}

pub async fn revoke_by_token(pool: &PgPool, raw_token: &str) -> Result<(), AppError> {
    let token_hash = hash_token(raw_token);

    // Look up the family_id for this token
    let family_id = infra::repos::refresh_tokens::find_family_id_by_hash(pool, &token_hash)
        .await
        .map_err(|e| AppError::Internal(format!("DB error: {}", e)))?;

    if let Some(family_id) = family_id {
        infra::repos::refresh_tokens::revoke_family(pool, family_id)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to revoke family: {}", e)))?;
    }

    Ok(())
}
