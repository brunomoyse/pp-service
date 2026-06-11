//! Audited manual point adjustments for a league.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgExecutor, Result as SqlxResult};
use uuid::Uuid;

const COLS: &str = "id, config_id, club_player_id, points_delta, reason, created_by, created_at";

#[derive(Debug, Clone, FromRow)]
pub struct LeaderboardAdjustmentRow {
    pub id: Uuid,
    pub config_id: Uuid,
    pub club_player_id: Uuid,
    pub points_delta: i32,
    pub reason: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    config_id: Uuid,
    club_player_id: Uuid,
    points_delta: i32,
    reason: &str,
    created_by: Option<Uuid>,
) -> SqlxResult<LeaderboardAdjustmentRow> {
    sqlx::query_as::<_, LeaderboardAdjustmentRow>(&format!(
        "INSERT INTO leaderboard_adjustments \
         (config_id, club_player_id, points_delta, reason, created_by) \
         VALUES ($1, $2, $3, $4, $5) RETURNING {COLS}"
    ))
    .bind(config_id)
    .bind(club_player_id)
    .bind(points_delta)
    .bind(reason)
    .bind(created_by)
    .fetch_one(executor)
    .await
}

pub async fn list_by_config<'e>(
    executor: impl PgExecutor<'e>,
    config_id: Uuid,
) -> SqlxResult<Vec<LeaderboardAdjustmentRow>> {
    sqlx::query_as::<_, LeaderboardAdjustmentRow>(&format!(
        "SELECT {COLS} FROM leaderboard_adjustments WHERE config_id = $1 \
         ORDER BY created_at DESC"
    ))
    .bind(config_id)
    .fetch_all(executor)
    .await
}

pub async fn delete<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> SqlxResult<bool> {
    let result = sqlx::query("DELETE FROM leaderboard_adjustments WHERE id = $1")
        .bind(id)
        .execute(executor)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Total manual delta per club_player for a league (added to computed points).
pub async fn sum_by_player<'e>(
    executor: impl PgExecutor<'e>,
    config_id: Uuid,
) -> SqlxResult<HashMap<Uuid, i64>> {
    let rows: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT club_player_id, COALESCE(SUM(points_delta), 0)::bigint \
         FROM leaderboard_adjustments WHERE config_id = $1 \
         GROUP BY club_player_id",
    )
    .bind(config_id)
    .fetch_all(executor)
    .await?;
    Ok(rows.into_iter().collect())
}
