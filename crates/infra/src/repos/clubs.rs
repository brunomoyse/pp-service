use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::ClubRow;

pub async fn list<'e>(executor: impl PgExecutor<'e>) -> SqlxResult<Vec<ClubRow>> {
    sqlx::query_as::<_, ClubRow>(
        r#"
        SELECT id, name, city, postal_code, province, country, created_at, updated_at
        FROM clubs
        ORDER BY name ASC
        "#,
    )
    .fetch_all(executor)
    .await
}

/// Distinct, non-null province slugs that at least one club resolves to —
/// the set a province leaderboard filter can offer.
pub async fn list_provinces<'e>(executor: impl PgExecutor<'e>) -> SqlxResult<Vec<String>> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT DISTINCT province
        FROM clubs
        WHERE province IS NOT NULL
        ORDER BY province ASC
        "#,
    )
    .fetch_all(executor)
    .await
}

pub async fn get_by_id<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> SqlxResult<Option<ClubRow>> {
    sqlx::query_as::<_, ClubRow>(
        r#"
        SELECT id, name, city, postal_code, province, country, created_at, updated_at
        FROM clubs
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}
