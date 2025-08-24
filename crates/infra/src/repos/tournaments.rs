use crate::{db::Db, models::TournamentRow, pagination::LimitOffset};
use chrono::{DateTime, Utc};
use sqlx::Result as SqlxResult;
use std::str::FromStr;
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
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "tournament_live_status", rename_all = "snake_case")]
pub enum TournamentLiveStatus {
    NotStarted,
    RegistrationOpen,
    LateRegistration,
    InProgress,
    Break,
    FinalTable,
    Finished,
}

impl TournamentLiveStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TournamentLiveStatus::NotStarted => "not_started",
            TournamentLiveStatus::RegistrationOpen => "registration_open",
            TournamentLiveStatus::LateRegistration => "late_registration",
            TournamentLiveStatus::InProgress => "in_progress",
            TournamentLiveStatus::Break => "break",
            TournamentLiveStatus::FinalTable => "final_table",
            TournamentLiveStatus::Finished => "finished",
        }
    }
}

impl FromStr for TournamentLiveStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "not_started" => Ok(TournamentLiveStatus::NotStarted),
            "registration_open" => Ok(TournamentLiveStatus::RegistrationOpen),
            "late_registration" => Ok(TournamentLiveStatus::LateRegistration),
            "in_progress" => Ok(TournamentLiveStatus::InProgress),
            "break" => Ok(TournamentLiveStatus::Break),
            "final_table" => Ok(TournamentLiveStatus::FinalTable),
            "finished" => Ok(TournamentLiveStatus::Finished),
            _ => Err(format!("Unknown tournament live status: {}", s)),
        }
    }
}

#[derive(Clone)]
pub struct TournamentRepo {
    pool: Db,
}

impl TournamentRepo {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }

    pub async fn get(&self, id: Uuid) -> SqlxResult<Option<TournamentRow>> {
        sqlx::query_as::<_, TournamentRow>(
            r#"
            SELECT id, club_id, name, description, start_time, end_time,
                   buy_in_cents, seat_cap, live_status, created_at, updated_at
            FROM tournaments
            WHERE id = $1
            "#,
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
                   buy_in_cents, seat_cap, live_status, created_at, updated_at
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
            ORDER BY created_at DESC
            LIMIT $5 OFFSET $6
            "#
        )
            .bind(filter.club_id)
            .bind(filter.from)
            .bind(filter.to)
            .bind(filter.status.map(|s| match s {
                TournamentStatus::Upcoming => "upcoming",
                TournamentStatus::InProgress => "in_progress", 
                TournamentStatus::Completed => "completed",
            }))
            .bind(p.limit)
            .bind(p.offset)
            .fetch_all(&self.pool)
            .await
    }

    /// Update tournament live status
    pub async fn update_live_status(
        &self,
        id: Uuid,
        live_status: TournamentLiveStatus,
    ) -> SqlxResult<Option<TournamentRow>> {
        sqlx::query_as::<_, TournamentRow>(
            r#"
            UPDATE tournaments
            SET live_status = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, club_id, name, description, start_time, end_time,
                     buy_in_cents, seat_cap, live_status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(live_status.as_str())
        .fetch_optional(&self.pool)
        .await
    }

    /// Get tournaments by live status
    pub async fn get_by_live_status(
        &self,
        live_status: TournamentLiveStatus,
    ) -> SqlxResult<Vec<TournamentRow>> {
        sqlx::query_as::<_, TournamentRow>(
            r#"
            SELECT id, club_id, name, description, start_time, end_time,
                   buy_in_cents, seat_cap, live_status, created_at, updated_at
            FROM tournaments
            WHERE live_status = $1
            ORDER BY start_time ASC
            "#,
        )
        .bind(live_status.as_str())
        .fetch_all(&self.pool)
        .await
    }

    /// Get live tournaments (in progress, break, or final table)
    pub async fn get_live_tournaments(&self) -> SqlxResult<Vec<TournamentRow>> {
        sqlx::query_as::<_, TournamentRow>(
            r#"
            SELECT id, club_id, name, description, start_time, end_time,
                   buy_in_cents, seat_cap, live_status, created_at, updated_at
            FROM tournaments
            WHERE live_status IN ('in_progress', 'break', 'final_table')
            ORDER BY start_time ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
    }
}
