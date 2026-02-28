use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PasswordResetTokenRow {
    pub id: Uuid,
    pub token_hash: String,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub async fn create(
    pool: &PgPool,
    token_hash: &str,
    user_id: Uuid,
    expires_at: DateTime<Utc>,
) -> Result<Uuid, sqlx::Error> {
    let row = sqlx::query(
        "INSERT INTO password_reset_tokens (token_hash, user_id, expires_at) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(token_hash)
    .bind(user_id)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;

    Ok(row.get("id"))
}

pub async fn find_valid_by_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<PasswordResetTokenRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, token_hash, user_id, expires_at, used_at, created_at FROM password_reset_tokens WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| PasswordResetTokenRow {
        id: r.get("id"),
        token_hash: r.get("token_hash"),
        user_id: r.get("user_id"),
        expires_at: r.get("expires_at"),
        used_at: r.get("used_at"),
        created_at: r.get("created_at"),
    }))
}

pub async fn mark_used(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE password_reset_tokens SET used_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn invalidate_for_user(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE password_reset_tokens SET used_at = now() WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_expired(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM password_reset_tokens WHERE expires_at < now()")
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}
