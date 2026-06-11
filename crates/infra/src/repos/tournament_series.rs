//! Multi-day series: CRUD over `tournament_series`.
//!
//! Each flight and the final day are ordinary `tournaments` rows linked by
//! `series_id` (see `repos::tournaments`). This module owns only the thin event
//! parent plus a couple of series-wide aggregates used by the GraphQL layer.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgExecutor, Result as SqlxResult};
use uuid::Uuid;

const COLS: &str = "id, club_id, title, best_stack_forward, created_at, updated_at";

#[derive(Debug, Clone, FromRow)]
pub struct TournamentSeriesRow {
    pub id: Uuid,
    pub club_id: Uuid,
    pub title: String,
    pub best_stack_forward: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    title: String,
    best_stack_forward: bool,
) -> SqlxResult<TournamentSeriesRow> {
    sqlx::query_as::<_, TournamentSeriesRow>(&format!(
        "INSERT INTO tournament_series (club_id, title, best_stack_forward) \
         VALUES ($1, $2, $3) RETURNING {COLS}"
    ))
    .bind(club_id)
    .bind(title)
    .bind(best_stack_forward)
    .fetch_one(executor)
    .await
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<TournamentSeriesRow>> {
    sqlx::query_as::<_, TournamentSeriesRow>(&format!(
        "SELECT {COLS} FROM tournament_series WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn list_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
) -> SqlxResult<Vec<TournamentSeriesRow>> {
    sqlx::query_as::<_, TournamentSeriesRow>(&format!(
        "SELECT {COLS} FROM tournament_series WHERE club_id = $1 ORDER BY created_at DESC"
    ))
    .bind(club_id)
    .fetch_all(executor)
    .await
}

/// Distinct entrants across every flight of the series (paid entries only).
/// This is the field size used to score the final day as one event.
pub async fn distinct_entrant_count<'e>(
    executor: impl PgExecutor<'e>,
    series_id: Uuid,
) -> SqlxResult<i64> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(DISTINCT te.club_player_id)
        FROM tournament_entries te
        JOIN tournaments t ON t.id = te.tournament_id
        WHERE t.series_id = $1
          AND te.entry_type NOT IN ('voucher', 'bonus')
        "#,
    )
    .bind(series_id)
    .fetch_one(executor)
    .await
}

/// The final-day tournament id for a series, if one exists.
pub async fn final_day_id<'e>(
    executor: impl PgExecutor<'e>,
    series_id: Uuid,
) -> SqlxResult<Option<Uuid>> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM tournaments WHERE series_id = $1 AND is_final_day = TRUE LIMIT 1",
    )
    .bind(series_id)
    .fetch_optional(executor)
    .await
}
