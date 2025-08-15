use crate::{db::Db, models::TournamentRow, pagination::LimitOffset};
use chrono::{DateTime, Utc};
use sqlx::Result as SqlxResult;
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct TournamentFilter {
    pub club_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub status: Option<TournamentStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TournamentStatus {
    Upcoming,
    Ongoing,
    Ended,
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
              AND (
                $4::text IS NULL 
                OR ($4 = 'upcoming' AND start_time > NOW())
                OR ($4 = 'ongoing' AND start_time <= NOW() AND (end_time IS NULL OR end_time > NOW()))
                OR ($4 = 'ended' AND end_time IS NOT NULL AND end_time <= NOW())
              )
            ORDER BY start_time ASC
            LIMIT $5 OFFSET $6
            "#
        )
            .bind(filter.club_id)
            .bind(filter.from)
            .bind(filter.to)
            .bind(filter.status.map(|s| match s {
                TournamentStatus::Upcoming => "upcoming",
                TournamentStatus::Ongoing => "ongoing", 
                TournamentStatus::Ended => "ended",
            }))
            .bind(p.limit)
            .bind(p.offset)
            .fetch_all(&self.pool)
            .await
    }
}