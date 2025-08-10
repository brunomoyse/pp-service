use crate::{db::Db, models::TournamentRow, pagination::LimitOffset};
use chrono::{DateTime, Utc};
use sqlx::Result as SqlxResult;
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct TournamentFilter {
    pub club_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct TournamentRepo {
    pool: Db,
}

impl TournamentRepo {
    pub fn new(pool: Db) -> Self { Self { pool } }

    pub async fn get(&self, id: Uuid) -> SqlxResult<Option<TournamentRow>> {
        sqlx::query_as::<_, TournamentRow>(
            r#"
            SELECT id, club_id, name, description, start_time, end_time,
                   buy_in_cents, seat_cap, location, created_at, updated_at
            FROM tournaments
            WHERE id = $1
            "#
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list(
        &self,
        filter: TournamentFilter,
        page: Option<LimitOffset>,
    ) -> SqlxResult<Vec<TournamentRow>> {
        let p = page.unwrap_or_default();

        // Dynamic WHERE using COALESCE pattern to keep a single prepared statement
        sqlx::query_as::<_, TournamentRow>(
            r#"
            SELECT id, club_id, name, description, start_time, end_time,
                   buy_in_cents, seat_cap, location, created_at, updated_at
            FROM tournaments
            WHERE ($1::uuid IS NULL OR club_id = $1)
              AND ($2::timestamptz IS NULL OR start_time >= $2)
              AND ($3::timestamptz IS NULL OR start_time <= $3)
            ORDER BY start_time ASC
            LIMIT $4 OFFSET $5
            "#
        )
            .bind(filter.club_id)
            .bind(filter.from)
            .bind(filter.to)
            .bind(p.limit)
            .bind(p.offset)
            .fetch_all(&self.pool)
            .await
    }
}