use crate::{db::Db, models::{TournamentRow, TournamentStateRow}, pagination::LimitOffset};
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

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "not_started" => Some(TournamentLiveStatus::NotStarted),
            "registration_open" => Some(TournamentLiveStatus::RegistrationOpen),
            "late_registration" => Some(TournamentLiveStatus::LateRegistration),
            "in_progress" => Some(TournamentLiveStatus::InProgress),
            "break" => Some(TournamentLiveStatus::Break),
            "final_table" => Some(TournamentLiveStatus::FinalTable),
            "finished" => Some(TournamentLiveStatus::Finished),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateTournamentState {
    pub current_level: Option<i32>,
    pub players_remaining: Option<i32>,
    pub break_until: Option<DateTime<Utc>>,
    pub current_small_blind: Option<i32>,
    pub current_big_blind: Option<i32>,
    pub current_ante: Option<i32>,
    pub level_started_at: Option<DateTime<Utc>>,
    pub level_duration_minutes: Option<i32>,
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
                   buy_in_cents, seat_cap, live_status, created_at, updated_at
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

    /// Update tournament live status
    pub async fn update_live_status(&self, id: Uuid, live_status: TournamentLiveStatus) -> SqlxResult<Option<TournamentRow>> {
        sqlx::query_as::<_, TournamentRow>(
            r#"
            UPDATE tournaments
            SET live_status = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, club_id, name, description, start_time, end_time,
                     buy_in_cents, seat_cap, live_status, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(live_status.as_str())
        .fetch_optional(&self.pool)
        .await
    }

    /// Get tournament state
    pub async fn get_state(&self, tournament_id: Uuid) -> SqlxResult<Option<TournamentStateRow>> {
        sqlx::query_as::<_, TournamentStateRow>(
            r#"
            SELECT id, tournament_id, current_level, players_remaining,
                   break_until, current_small_blind, current_big_blind,
                   current_ante, level_started_at, level_duration_minutes,
                   created_at, updated_at
            FROM tournament_state
            WHERE tournament_id = $1
            "#
        )
        .bind(tournament_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Create or update tournament state
    pub async fn upsert_state(&self, tournament_id: Uuid, data: UpdateTournamentState) -> SqlxResult<TournamentStateRow> {
        sqlx::query_as::<_, TournamentStateRow>(
            r#"
            INSERT INTO tournament_state (
                tournament_id, current_level, players_remaining, break_until,
                current_small_blind, current_big_blind, current_ante,
                level_started_at, level_duration_minutes
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (tournament_id)
            DO UPDATE SET
                current_level = COALESCE($2, tournament_state.current_level),
                players_remaining = COALESCE($3, tournament_state.players_remaining),
                break_until = $4,
                current_small_blind = COALESCE($5, tournament_state.current_small_blind),
                current_big_blind = COALESCE($6, tournament_state.current_big_blind),
                current_ante = COALESCE($7, tournament_state.current_ante),
                level_started_at = COALESCE($8, tournament_state.level_started_at),
                level_duration_minutes = COALESCE($9, tournament_state.level_duration_minutes),
                updated_at = NOW()
            RETURNING id, tournament_id, current_level, players_remaining,
                     break_until, current_small_blind, current_big_blind,
                     current_ante, level_started_at, level_duration_minutes,
                     created_at, updated_at
            "#
        )
        .bind(tournament_id)
        .bind(data.current_level)
        .bind(data.players_remaining)
        .bind(data.break_until)
        .bind(data.current_small_blind)
        .bind(data.current_big_blind)
        .bind(data.current_ante)
        .bind(data.level_started_at)
        .bind(data.level_duration_minutes)
        .fetch_one(&self.pool)
        .await
    }

    /// Get tournaments by live status
    pub async fn get_by_live_status(&self, live_status: TournamentLiveStatus) -> SqlxResult<Vec<TournamentRow>> {
        sqlx::query_as::<_, TournamentRow>(
            r#"
            SELECT id, club_id, name, description, start_time, end_time,
                   buy_in_cents, seat_cap, live_status, created_at, updated_at
            FROM tournaments
            WHERE live_status = $1
            ORDER BY start_time ASC
            "#
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
            "#
        )
        .fetch_all(&self.pool)
        .await
    }
}