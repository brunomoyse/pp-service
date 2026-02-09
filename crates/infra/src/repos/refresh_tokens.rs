use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct RefreshTokenRow {
    pub id: Uuid,
    pub token_hash: String,
    pub user_id: Uuid,
    pub family_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub async fn create(
    pool: &PgPool,
    token_hash: &str,
    user_id: Uuid,
    family_id: Uuid,
    expires_at: DateTime<Utc>,
) -> Result<Uuid, sqlx::Error> {
    let row = sqlx::query(
        "INSERT INTO refresh_tokens (token_hash, user_id, family_id, expires_at) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(token_hash)
    .bind(user_id)
    .bind(family_id)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;

    Ok(row.get("id"))
}

pub async fn find_by_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<RefreshTokenRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, token_hash, user_id, family_id, expires_at, revoked_at, created_at FROM refresh_tokens WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| RefreshTokenRow {
        id: r.get("id"),
        token_hash: r.get("token_hash"),
        user_id: r.get("user_id"),
        family_id: r.get("family_id"),
        expires_at: r.get("expires_at"),
        revoked_at: r.get("revoked_at"),
        created_at: r.get("created_at"),
    }))
}

pub async fn is_revoked(pool: &PgPool, token_hash: &str) -> Result<bool, sqlx::Error> {
    let row: Option<_> = sqlx::query(
        "SELECT 1 FROM refresh_tokens WHERE token_hash = $1 AND revoked_at IS NOT NULL",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}

pub async fn find_family_id_by_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query("SELECT family_id FROM refresh_tokens WHERE token_hash = $1")
        .bind(token_hash)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|r| r.get("family_id")))
}

pub async fn revoke(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE refresh_tokens SET revoked_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn revoke_family(pool: &PgPool, family_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = now() WHERE family_id = $1 AND revoked_at IS NULL",
    )
    .bind(family_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_expired(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < now()")
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}
