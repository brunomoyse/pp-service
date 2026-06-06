use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::{ScoutingMatchRow, ScoutingStatsRow};

/// Search the scouting pool by handle. Only users who opted into the pool
/// (consented to discoverability) are returned.
pub async fn search_pool<'e>(
    executor: impl PgExecutor<'e>,
    query: &str,
    limit: i64,
) -> SqlxResult<Vec<ScoutingMatchRow>> {
    let pattern = format!("%{query}%");
    sqlx::query_as::<_, ScoutingMatchRow>(
        "SELECT u.id AS user_id, COALESCE(u.username, u.first_name) AS handle \
         FROM users u \
         JOIN user_privacy_settings ups ON ups.app_user_id = u.id \
         WHERE ups.in_scouting_pool \
           AND (u.username ILIKE $1 OR u.first_name ILIKE $1) \
         ORDER BY handle ASC LIMIT $2",
    )
    .bind(pattern)
    .bind(limit)
    .fetch_all(executor)
    .await
}

/// A target's lifetime performance aggregates.
pub async fn profile_stats<'e>(
    executor: impl PgExecutor<'e>,
    target_id: Uuid,
) -> SqlxResult<ScoutingStatsRow> {
    sqlx::query_as::<_, ScoutingStatsRow>(
        "SELECT COUNT(*) AS tournaments, \
                COALESCE(SUM(CASE WHEN tr.prize_cents > 0 THEN 1 ELSE 0 END), 0) AS itm_count, \
                MIN(tr.final_position) AS best_finish, \
                COALESCE(SUM(tr.prize_cents), 0) - COALESCE(SUM(t.buy_in_cents), 0) AS net_cents \
         FROM tournament_results tr JOIN tournaments t ON t.id = tr.tournament_id \
         WHERE tr.user_id = $1",
    )
    .bind(target_id)
    .fetch_one(executor)
    .await
}

/// Look up a pool member's handle (None if they aren't in the pool).
pub async fn pool_handle<'e>(
    executor: impl PgExecutor<'e>,
    target_id: Uuid,
) -> SqlxResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT COALESCE(u.username, u.first_name) AS handle \
         FROM users u JOIN user_privacy_settings ups ON ups.app_user_id = u.id \
         WHERE u.id = $1 AND ups.in_scouting_pool",
    )
    .bind(target_id)
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|r| r.0))
}

/// Distinct profiles a searcher has viewed since `since` (the quota usage).
pub async fn distinct_targets_since<'e>(
    executor: impl PgExecutor<'e>,
    searcher_id: Uuid,
    since: DateTime<Utc>,
) -> SqlxResult<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT target_id) FROM scouting_lookup \
         WHERE searcher_id = $1 AND created_at >= $2",
    )
    .bind(searcher_id)
    .bind(since)
    .fetch_one(executor)
    .await?;
    Ok(row.0)
}

/// Whether the searcher has already viewed this target within the window (a
/// repeat view is free — it doesn't consume new quota).
pub async fn has_looked_since<'e>(
    executor: impl PgExecutor<'e>,
    searcher_id: Uuid,
    target_id: Uuid,
    since: DateTime<Utc>,
) -> SqlxResult<bool> {
    let row: (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM scouting_lookup \
         WHERE searcher_id = $1 AND target_id = $2 AND created_at >= $3)",
    )
    .bind(searcher_id)
    .bind(target_id)
    .bind(since)
    .fetch_one(executor)
    .await?;
    Ok(row.0)
}

pub async fn record_lookup<'e>(
    executor: impl PgExecutor<'e>,
    searcher_id: Uuid,
    target_id: Uuid,
) -> SqlxResult<()> {
    sqlx::query("INSERT INTO scouting_lookup (searcher_id, target_id) VALUES ($1, $2)")
        .bind(searcher_id)
        .bind(target_id)
        .execute(executor)
        .await?;
    Ok(())
}
