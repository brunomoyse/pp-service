use crate::{models::TournamentRow, pagination::LimitOffset};
use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, Result as SqlxResult};
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

#[derive(Debug, Clone)]
pub struct CreateTournamentData {
    pub club_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub buy_in_cents: i32,
    pub seat_cap: Option<i32>,
    pub early_bird_bonus_chips: Option<i32>,
    pub late_registration_level: Option<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateTournamentData {
    pub name: Option<String>,
    pub description: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub buy_in_cents: Option<i32>,
    pub seat_cap: Option<i32>,
    pub early_bird_bonus_chips: Option<i32>,
    pub late_registration_level: Option<i32>,
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<TournamentRow>> {
    sqlx::query_as::<_, TournamentRow>(
        r#"
        SELECT id, club_id, name, description, start_time, end_time,
               buy_in_cents, seat_cap, live_status, early_bird_bonus_chips,
               late_registration_level, created_at, updated_at
        FROM tournaments
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn list<'e>(
    executor: impl PgExecutor<'e>,
    filter: TournamentFilter,
    page: Option<LimitOffset>,
) -> SqlxResult<Vec<TournamentRow>> {
    let p = page.unwrap_or_default();

    sqlx::query_as::<_, TournamentRow>(
        r#"
        SELECT id, club_id, name, description, start_time, end_time,
               buy_in_cents, seat_cap, live_status, early_bird_bonus_chips,
               late_registration_level, created_at, updated_at
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
        "#,
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
    .fetch_all(executor)
    .await
}

pub async fn count<'e>(executor: impl PgExecutor<'e>, filter: TournamentFilter) -> SqlxResult<i64> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
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
        "#,
    )
    .bind(filter.club_id)
    .bind(filter.from)
    .bind(filter.to)
    .bind(filter.status.map(|s| match s {
        TournamentStatus::Upcoming => "upcoming",
        TournamentStatus::InProgress => "in_progress",
        TournamentStatus::Completed => "completed",
    }))
    .fetch_one(executor)
    .await
}

pub async fn update_live_status<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    live_status: TournamentLiveStatus,
) -> SqlxResult<Option<TournamentRow>> {
    sqlx::query_as::<_, TournamentRow>(
        r#"
        UPDATE tournaments
        SET live_status = $2::tournament_live_status,
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, club_id, name, description, start_time, end_time,
                 buy_in_cents, seat_cap, live_status, early_bird_bonus_chips,
                 late_registration_level, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(live_status.as_str())
    .fetch_optional(executor)
    .await
}

pub async fn list_by_live_status<'e>(
    executor: impl PgExecutor<'e>,
    live_status: TournamentLiveStatus,
) -> SqlxResult<Vec<TournamentRow>> {
    sqlx::query_as::<_, TournamentRow>(
        r#"
        SELECT id, club_id, name, description, start_time, end_time,
               buy_in_cents, seat_cap, live_status, early_bird_bonus_chips,
               late_registration_level, created_at, updated_at
        FROM tournaments
        WHERE live_status = $1
        ORDER BY start_time ASC
        "#,
    )
    .bind(live_status.as_str())
    .fetch_all(executor)
    .await
}

pub async fn list_live<'e>(executor: impl PgExecutor<'e>) -> SqlxResult<Vec<TournamentRow>> {
    sqlx::query_as::<_, TournamentRow>(
        r#"
        SELECT id, club_id, name, description, start_time, end_time,
               buy_in_cents, seat_cap, live_status, early_bird_bonus_chips,
               late_registration_level, created_at, updated_at
        FROM tournaments
        WHERE live_status IN ('in_progress', 'break', 'final_table')
        ORDER BY start_time ASC
        "#,
    )
    .fetch_all(executor)
    .await
}

pub async fn list_starting_soon<'e>(
    executor: impl PgExecutor<'e>,
    within_minutes: i32,
) -> SqlxResult<Vec<TournamentRow>> {
    sqlx::query_as::<_, TournamentRow>(
        r#"
        SELECT id, club_id, name, description, start_time, end_time,
               buy_in_cents, seat_cap, live_status, early_bird_bonus_chips,
               late_registration_level, created_at, updated_at
        FROM tournaments
        WHERE live_status IN ('not_started', 'registration_open')
          AND start_time > NOW()
          AND start_time <= NOW() + ($1 || ' minutes')::INTERVAL
        ORDER BY start_time ASC
        "#,
    )
    .bind(within_minutes)
    .fetch_all(executor)
    .await
}

pub async fn get_by_ids<'e>(
    executor: impl PgExecutor<'e>,
    ids: &[Uuid],
) -> SqlxResult<Vec<TournamentRow>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_as::<_, TournamentRow>(
        r#"
        SELECT id, club_id, name, description, start_time, end_time,
               buy_in_cents, seat_cap, live_status, early_bird_bonus_chips,
               late_registration_level, created_at, updated_at
        FROM tournaments
        WHERE id = ANY($1::uuid[])
        "#,
    )
    .bind(ids)
    .fetch_all(executor)
    .await
}

pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    data: CreateTournamentData,
) -> SqlxResult<TournamentRow> {
    sqlx::query_as::<_, TournamentRow>(
        r#"
        INSERT INTO tournaments (club_id, name, description, start_time, end_time,
                                 buy_in_cents, seat_cap, early_bird_bonus_chips,
                                 late_registration_level)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id, club_id, name, description, start_time, end_time,
                  buy_in_cents, seat_cap, live_status, early_bird_bonus_chips,
                  late_registration_level, created_at, updated_at
        "#,
    )
    .bind(data.club_id)
    .bind(data.name)
    .bind(data.description)
    .bind(data.start_time)
    .bind(data.end_time)
    .bind(data.buy_in_cents)
    .bind(data.seat_cap)
    .bind(data.early_bird_bonus_chips)
    .bind(data.late_registration_level)
    .fetch_one(executor)
    .await
}

pub async fn update<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    data: UpdateTournamentData,
) -> SqlxResult<Option<TournamentRow>> {
    sqlx::query_as::<_, TournamentRow>(
        r#"
        UPDATE tournaments
        SET name = COALESCE($2, name),
            description = COALESCE($3, description),
            start_time = COALESCE($4, start_time),
            end_time = COALESCE($5, end_time),
            buy_in_cents = COALESCE($6, buy_in_cents),
            seat_cap = COALESCE($7, seat_cap),
            early_bird_bonus_chips = COALESCE($8, early_bird_bonus_chips),
            late_registration_level = COALESCE($9, late_registration_level),
            updated_at = NOW()
        WHERE id = $1 AND live_status != 'finished'
        RETURNING id, club_id, name, description, start_time, end_time,
                  buy_in_cents, seat_cap, live_status, early_bird_bonus_chips,
                  late_registration_level, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(data.name)
    .bind(data.description)
    .bind(data.start_time)
    .bind(data.end_time)
    .bind(data.buy_in_cents)
    .bind(data.seat_cap)
    .bind(data.early_bird_bonus_chips)
    .bind(data.late_registration_level)
    .fetch_optional(executor)
    .await
}

pub async fn list_stale<'e>(
    executor: impl PgExecutor<'e>,
    max_hours: i32,
) -> SqlxResult<Vec<TournamentRow>> {
    sqlx::query_as::<_, TournamentRow>(
        r#"
        SELECT id, club_id, name, description, start_time, end_time,
               buy_in_cents, seat_cap, live_status, early_bird_bonus_chips,
               late_registration_level, created_at, updated_at
        FROM tournaments
        WHERE live_status IN ('in_progress', 'late_registration', 'break', 'final_table')
          AND updated_at < NOW() - ($1 || ' hours')::INTERVAL
        ORDER BY updated_at ASC
        "#,
    )
    .bind(max_hours)
    .fetch_all(executor)
    .await
}

pub async fn auto_finish<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<TournamentRow>> {
    sqlx::query_as::<_, TournamentRow>(
        r#"
        UPDATE tournaments
        SET live_status = 'finished',
            end_time = NOW(),
            updated_at = NOW()
        WHERE id = $1 AND live_status != 'finished'
        RETURNING id, club_id, name, description, start_time, end_time,
                  buy_in_cents, seat_cap, live_status, early_bird_bonus_chips,
                  late_registration_level, created_at, updated_at
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}
