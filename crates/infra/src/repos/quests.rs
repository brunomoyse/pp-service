use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::QuestCompletionRow;

/// Count a player's check-ins within a [start, end) window (any club).
pub async fn check_in_count<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> SqlxResult<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM check_in \
         WHERE app_user_id = $1 AND checked_in_at >= $2 AND checked_in_at < $3",
    )
    .bind(app_user_id)
    .bind(start)
    .bind(end)
    .fetch_one(executor)
    .await?;
    Ok(row.0)
}

/// Count the distinct clubs a player checked in at within a window.
pub async fn distinct_clubs<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> SqlxResult<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT club_id) FROM check_in \
         WHERE app_user_id = $1 AND checked_in_at >= $2 AND checked_in_at < $3",
    )
    .bind(app_user_id)
    .bind(start)
    .bind(end)
    .fetch_one(executor)
    .await?;
    Ok(row.0)
}

/// The player's quest completions for a given week.
pub async fn completions_for_week<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    week_start: NaiveDate,
) -> SqlxResult<Vec<QuestCompletionRow>> {
    sqlx::query_as::<_, QuestCompletionRow>(
        "SELECT id, app_user_id, quest_code, week_start, xp_awarded, completed_at \
         FROM quest_completion WHERE app_user_id = $1 AND week_start = $2",
    )
    .bind(app_user_id)
    .bind(week_start)
    .fetch_all(executor)
    .await
}

/// Record a claimed quest. Idempotent per (user, quest, week): returns the row
/// only when newly inserted, None if it was already claimed.
pub async fn claim<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    quest_code: &str,
    week_start: NaiveDate,
    xp_awarded: i32,
) -> SqlxResult<Option<QuestCompletionRow>> {
    sqlx::query_as::<_, QuestCompletionRow>(
        "INSERT INTO quest_completion (app_user_id, quest_code, week_start, xp_awarded) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (app_user_id, quest_code, week_start) DO NOTHING \
         RETURNING id, app_user_id, quest_code, week_start, xp_awarded, completed_at",
    )
    .bind(app_user_id)
    .bind(quest_code)
    .bind(week_start)
    .bind(xp_awarded)
    .fetch_optional(executor)
    .await
}

/// Sum of quest XP a player earned within a [start, end) window — folds into the
/// season-pass total.
pub async fn xp_in_window<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> SqlxResult<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(xp_awarded), 0) FROM quest_completion \
         WHERE app_user_id = $1 AND completed_at >= $2 AND completed_at < $3",
    )
    .bind(app_user_id)
    .bind(start)
    .bind(end)
    .fetch_one(executor)
    .await?;
    Ok(row.0)
}
