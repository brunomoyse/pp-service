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
    pub remember_me: bool,
}

pub fn hash_token(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    // sha2 0.11's `finalize()` returns `hybrid_array::Array`, which (unlike the
    // old `GenericArray`) doesn't implement `LowerHex`. Format each byte as
    // two lowercase hex digits to keep the output identical to the previous
    // `format!("{:x}", ..)` so existing stored token hashes still match.
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub async fn create_refresh_token(
    pool: &PgPool,
    user_id: Uuid,
    expiration_days: u64,
    remember_me: bool,
) -> Result<String, AppError> {
    let raw_token: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();

    let token_hash = hash_token(&raw_token);
    let family_id = Uuid::new_v4();
    let expires_at = Utc::now() + Duration::days(expiration_days as i64);

    infra::repos::refresh_tokens::create(
        pool,
        &token_hash,
        user_id,
        family_id,
        expires_at,
        remember_me,
    )
    .await
    .map_err(|e| AppError::Internal(format!("Failed to create refresh token: {}", e)))?;

    // Activity heartbeat (issued at login). Best-effort: never block auth on it.
    touch_last_seen(pool, user_id).await;

    Ok(raw_token)
}

/// Update the user's `last_seen_at` without ever failing the surrounding auth
/// flow — a retention-signal write is not worth rejecting a login/refresh over.
async fn touch_last_seen(pool: &PgPool, user_id: Uuid) {
    if let Err(e) = infra::repos::users::touch_last_seen(pool, user_id).await {
        tracing::warn!("Failed to update last_seen_at for {user_id}: {e}");
    }
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
                token_row.remember_me,
            )
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create new token: {}", e)))?;

            // Activity heartbeat (active session refresh). Best-effort.
            touch_last_seen(pool, token_row.user_id).await;

            Ok(RotateResult {
                user_id: token_row.user_id,
                new_raw_token: new_raw,
                remember_me: token_row.remember_me,
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
                    // Identify the affected account so this is actionable in
                    // security monitoring (and so a follow-up can email/push the
                    // user). Any token in the family carries the same user_id.
                    let user_id: Option<Uuid> = sqlx::query_scalar(
                        "SELECT user_id FROM refresh_tokens WHERE family_id = $1 LIMIT 1",
                    )
                    .bind(family_id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| AppError::Internal(format!("DB error: {}", e)))?;

                    infra::repos::refresh_tokens::revoke_family(pool, family_id)
                        .await
                        .map_err(|e| {
                            AppError::Internal(format!("Failed to revoke family: {}", e))
                        })?;
                    // Structured + targeted so an alerting pipeline can page on it.
                    // TODO(security): also email/push the user that all sessions
                    // were invalidated (needs the email/push service threaded in).
                    tracing::warn!(
                        target: "security",
                        family_id = %family_id,
                        user_id = ?user_id,
                        "Refresh token reuse detected; revoked entire token family \
                         (all sessions for this account invalidated)"
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
