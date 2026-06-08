use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Insert or refresh a device's Expo push token, claiming it for `user_id`.
///
/// Keyed on the token: re-registering the same physical device (same token)
/// under a different account reassigns it, so a shared device only ever
/// receives the currently signed-in user's notifications.
pub async fn upsert(
    pool: &PgPool,
    user_id: Uuid,
    token: &str,
    platform: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO device_tokens (user_id, token, platform) VALUES ($1, $2, $3) \
         ON CONFLICT (token) DO UPDATE SET user_id = EXCLUDED.user_id, \
         platform = EXCLUDED.platform, updated_at = now()",
    )
    .bind(user_id)
    .bind(token)
    .bind(platform)
    .execute(pool)
    .await?;

    Ok(())
}

/// Remove a token for a specific user (logout on this device). Scoped by user
/// so a caller can only drop a token currently registered to themselves.
pub async fn delete_for_user(pool: &PgPool, user_id: Uuid, token: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM device_tokens WHERE user_id = $1 AND token = $2")
        .bind(user_id)
        .bind(token)
        .execute(pool)
        .await?;

    Ok(())
}

/// All Expo push tokens registered for a user (one per device).
pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query("SELECT token FROM device_tokens WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(pool)
        .await?;

    Ok(rows.iter().map(|r| r.get::<String, _>("token")).collect())
}

/// Prune tokens Expo reports as dead (e.g. `DeviceNotRegistered`).
pub async fn delete_tokens(pool: &PgPool, tokens: &[String]) -> Result<(), sqlx::Error> {
    if tokens.is_empty() {
        return Ok(());
    }
    sqlx::query("DELETE FROM device_tokens WHERE token = ANY($1)")
        .bind(tokens)
        .execute(pool)
        .await?;

    Ok(())
}
