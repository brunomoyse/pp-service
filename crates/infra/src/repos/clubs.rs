use crate::{db::Db, models::ClubRow};
use sqlx::Result as SqlxResult;
use uuid::Uuid;

#[derive(Clone)]
pub struct ClubRepo {
    pool: Db,
}

impl ClubRepo {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self) -> SqlxResult<Vec<ClubRow>> {
        sqlx::query_as::<_, ClubRow>(
            r#"
            SELECT id, name, city, country, created_at, updated_at
            FROM clubs
            ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get(&self, id: Uuid) -> SqlxResult<Option<ClubRow>> {
        sqlx::query_as::<_, ClubRow>(
            r#"
            SELECT id, name, city, country, created_at, updated_at
            FROM clubs
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }
}
