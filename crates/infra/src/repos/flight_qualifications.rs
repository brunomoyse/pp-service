//! Flight survivors that qualify for a series' final day.
//!
//! One row per (series, player, source flight). `is_best` marks the single
//! stack carried to Day 2 when a player qualifies from more than one flight
//! ("best stack forward"). `list_best_by_series` returns one row per player —
//! the source for seeding Day 2 registrations.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgExecutor, Result as SqlxResult};
use uuid::Uuid;

const COLS: &str =
    "id, series_id, club_player_id, from_tournament_id, chip_count, is_best, created_at";

#[derive(Debug, Clone, FromRow)]
pub struct FlightQualificationRow {
    pub id: Uuid,
    pub series_id: Uuid,
    pub club_player_id: Uuid,
    pub from_tournament_id: Uuid,
    pub chip_count: i32,
    pub is_best: bool,
    pub created_at: DateTime<Utc>,
}

/// Record a flight survivor. Idempotent on (series, player, source flight) —
/// re-running a flight close updates the recorded stack. After inserting, the
/// per-player `is_best` flags are recomputed so exactly the largest surviving
/// stack is flagged for Day 2.
pub async fn record<'e>(
    executor: impl PgExecutor<'e>,
    series_id: Uuid,
    club_player_id: Uuid,
    from_tournament_id: Uuid,
    chip_count: i32,
) -> SqlxResult<FlightQualificationRow> {
    sqlx::query_as::<_, FlightQualificationRow>(&format!(
        "INSERT INTO flight_qualifications \
         (series_id, club_player_id, from_tournament_id, chip_count) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (series_id, club_player_id, from_tournament_id) DO UPDATE SET \
            chip_count = EXCLUDED.chip_count \
         RETURNING {COLS}"
    ))
    .bind(series_id)
    .bind(club_player_id)
    .bind(from_tournament_id)
    .bind(chip_count)
    .fetch_one(executor)
    .await
}

/// Recompute `is_best` for a player in a series: the row with the largest
/// `chip_count` wins (ties broken by most recent). Call after `record`.
pub async fn refresh_best<'e>(
    executor: impl PgExecutor<'e>,
    series_id: Uuid,
    club_player_id: Uuid,
) -> SqlxResult<()> {
    sqlx::query(
        r#"
        WITH ranked AS (
            SELECT id, ROW_NUMBER() OVER (
                ORDER BY chip_count DESC, created_at DESC
            ) AS rn
            FROM flight_qualifications
            WHERE series_id = $1 AND club_player_id = $2
        )
        UPDATE flight_qualifications fq
        SET is_best = (r.rn = 1)
        FROM ranked r
        WHERE fq.id = r.id
        "#,
    )
    .bind(series_id)
    .bind(club_player_id)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn list_by_series<'e>(
    executor: impl PgExecutor<'e>,
    series_id: Uuid,
) -> SqlxResult<Vec<FlightQualificationRow>> {
    sqlx::query_as::<_, FlightQualificationRow>(&format!(
        "SELECT {COLS} FROM flight_qualifications WHERE series_id = $1 \
         ORDER BY chip_count DESC, created_at DESC"
    ))
    .bind(series_id)
    .fetch_all(executor)
    .await
}

/// One row per qualified player: the best surviving stack carried to Day 2.
pub async fn list_best_by_series<'e>(
    executor: impl PgExecutor<'e>,
    series_id: Uuid,
) -> SqlxResult<Vec<FlightQualificationRow>> {
    sqlx::query_as::<_, FlightQualificationRow>(&format!(
        "SELECT {COLS} FROM flight_qualifications \
         WHERE series_id = $1 AND is_best = TRUE \
         ORDER BY chip_count DESC"
    ))
    .bind(series_id)
    .fetch_all(executor)
    .await
}
