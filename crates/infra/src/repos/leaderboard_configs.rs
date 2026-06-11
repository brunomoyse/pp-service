//! Configurable leaderboards / leagues: CRUD over `leaderboard_configs`.
//!
//! `formula_params` is stored as JSONB and surfaced as `serde_json::Value`; the
//! API layer types it into `ScoringFormula`. Points themselves are never stored
//! per-config — they are computed on read (see `tournament_results`).

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgExecutor, Result as SqlxResult};
use uuid::Uuid;

const COLS: &str = "id, club_id, name, formula_params, membership_mode, \
                    period_start, period_end, is_default, created_at, updated_at";

#[derive(Debug, Clone, FromRow)]
pub struct LeaderboardConfigRow {
    pub id: Uuid,
    pub club_id: Uuid,
    pub name: String,
    pub formula_params: serde_json::Value,
    pub membership_mode: String,
    pub period_start: Option<DateTime<Utc>>,
    pub period_end: Option<DateTime<Utc>>,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateLeaderboardConfigData {
    pub club_id: Uuid,
    pub name: String,
    pub formula_params: serde_json::Value,
    pub membership_mode: String,
    pub period_start: Option<DateTime<Utc>>,
    pub period_end: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateLeaderboardConfigData {
    pub name: Option<String>,
    pub formula_params: Option<serde_json::Value>,
    pub membership_mode: Option<String>,
    pub period_start: Option<DateTime<Utc>>,
    pub period_end: Option<DateTime<Utc>>,
}

pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    data: CreateLeaderboardConfigData,
) -> SqlxResult<LeaderboardConfigRow> {
    sqlx::query_as::<_, LeaderboardConfigRow>(&format!(
        "INSERT INTO leaderboard_configs \
         (club_id, name, formula_params, membership_mode, period_start, period_end) \
         VALUES ($1, $2, $3, COALESCE($4, 'all_in_period'), $5, $6) \
         RETURNING {COLS}"
    ))
    .bind(data.club_id)
    .bind(data.name)
    .bind(data.formula_params)
    .bind(data.membership_mode)
    .bind(data.period_start)
    .bind(data.period_end)
    .fetch_one(executor)
    .await
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<LeaderboardConfigRow>> {
    sqlx::query_as::<_, LeaderboardConfigRow>(&format!(
        "SELECT {COLS} FROM leaderboard_configs WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn list_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
) -> SqlxResult<Vec<LeaderboardConfigRow>> {
    sqlx::query_as::<_, LeaderboardConfigRow>(&format!(
        "SELECT {COLS} FROM leaderboard_configs WHERE club_id = $1 \
         ORDER BY is_default DESC, created_at DESC"
    ))
    .bind(club_id)
    .fetch_all(executor)
    .await
}

pub async fn update<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    data: UpdateLeaderboardConfigData,
) -> SqlxResult<Option<LeaderboardConfigRow>> {
    sqlx::query_as::<_, LeaderboardConfigRow>(&format!(
        "UPDATE leaderboard_configs SET \
            name = COALESCE($2, name), \
            formula_params = COALESCE($3, formula_params), \
            membership_mode = COALESCE($4, membership_mode), \
            period_start = COALESCE($5, period_start), \
            period_end = COALESCE($6, period_end), \
            updated_at = NOW() \
         WHERE id = $1 \
         RETURNING {COLS}"
    ))
    .bind(id)
    .bind(data.name)
    .bind(data.formula_params)
    .bind(data.membership_mode)
    .bind(data.period_start)
    .bind(data.period_end)
    .fetch_optional(executor)
    .await
}

pub async fn delete<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> SqlxResult<bool> {
    let result = sqlx::query("DELETE FROM leaderboard_configs WHERE id = $1")
        .bind(id)
        .execute(executor)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Make `config_id` the club's default league (and clear the flag on the others).
pub async fn set_default<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    config_id: Uuid,
) -> SqlxResult<()> {
    sqlx::query(
        "UPDATE leaderboard_configs SET is_default = (id = $2), updated_at = NOW() \
         WHERE club_id = $1",
    )
    .bind(club_id)
    .bind(config_id)
    .execute(executor)
    .await?;
    Ok(())
}
