use async_graphql::dataloader::Loader;
use infra::{db::Db, models::ClubRow, models::TournamentRow, models::UserRow};
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

// UserLoader - batch load users by ID
#[derive(Clone)]
pub struct UserLoader {
    pool: Db,
}

impl UserLoader {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

impl Loader<Uuid> for UserLoader {
    type Value = UserRow;
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

            let rows: Vec<UserRow> = sqlx::query_as::<_, UserRow>(
                r#"
                SELECT id, email, username, first_name, last_name, phone,
                       is_active, role, locale, created_at, updated_at
                FROM users
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

// TournamentLoader - batch load tournaments by ID
#[derive(Clone)]
pub struct TournamentLoader {
    pool: Db,
}

impl TournamentLoader {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

impl Loader<Uuid> for TournamentLoader {
    type Value = TournamentRow;
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

            let rows: Vec<TournamentRow> = sqlx::query_as::<_, TournamentRow>(
                r#"
                SELECT id, club_id, name, description, start_time, end_time,
                       buy_in_cents, rake_cents, seat_cap, live_status, early_bird_bonus_chips,
                       late_registration_level, created_at, updated_at
                FROM tournaments
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
