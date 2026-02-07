use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::ClubRow;

pub async fn list<'e>(executor: impl PgExecutor<'e>) -> SqlxResult<Vec<ClubRow>> {
    sqlx::query_as::<_, ClubRow>(
        r#"
        SELECT id, name, city, country, created_at, updated_at
        FROM clubs
        ORDER BY name ASC
        "#,
    )
    .fetch_all(executor)
    .await
}

pub async fn get_by_id<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> SqlxResult<Option<ClubRow>> {
    sqlx::query_as::<_, ClubRow>(
        r#"
        SELECT id, name, city, country, created_at, updated_at
        FROM clubs
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}
