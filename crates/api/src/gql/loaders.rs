use async_graphql::dataloader::Loader;
use infra::{db::Db, models::ClubRow};
use std::{collections::HashMap, future::Future, sync::Arc};
use uuid::Uuid;

#[derive(Clone)]
pub struct ClubLoader {
    pool: Db,
}

impl ClubLoader {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

impl Loader<Uuid> for ClubLoader {
    type Value = ClubRow;
    type Error = Arc<sqlx::Error>;

    fn load(
        &self,
        keys: &[Uuid],
    ) -> impl Future<Output = std::result::Result<HashMap<Uuid, Self::Value>, Self::Error>> + Send
    {
        let pool = self.pool.clone();
        let ids: Vec<Uuid> = keys.to_vec();

        async move {
            if ids.is_empty() {
                return Ok(HashMap::new());
            }

            let rows: Vec<ClubRow> = sqlx::query_as::<_, ClubRow>(
                r#"
                SELECT id, name, city, country, created_at, updated_at
                FROM clubs
                WHERE id = ANY($1::uuid[])
                "#,
            )
                .bind(&ids)
                .fetch_all(&pool)
                .await
                .map_err(Arc::new)?;

            Ok(rows.into_iter().map(|r| (r.id, r)).collect())
        }
    }
}