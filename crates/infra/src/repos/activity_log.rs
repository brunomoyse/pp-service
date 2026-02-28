use crate::models::TournamentActivityLogRow;
use sqlx::{PgPool, Postgres, Result as SqlxResult};
use uuid::Uuid;

/// Insert a new activity log entry and return it.
pub async fn log_activity(
    executor: impl sqlx::Executor<'_, Database = Postgres>,
    tournament_id: Uuid,
    event_category: &str,
    event_action: &str,
    actor_id: Option<Uuid>,
    subject_id: Option<Uuid>,
    metadata: serde_json::Value,
) -> SqlxResult<TournamentActivityLogRow> {
    sqlx::query_as::<_, TournamentActivityLogRow>(
        "INSERT INTO tournament_activity_log
         (tournament_id, event_category, event_action, actor_id, subject_id, metadata)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, tournament_id, event_category, event_action, actor_id, subject_id, event_time, metadata",
    )
    .bind(tournament_id)
    .bind(event_category)
    .bind(event_action)
    .bind(actor_id)
    .bind(subject_id)
    .bind(metadata)
    .fetch_one(executor)
    .await
}

/// List activity log entries for a tournament, newest first.
/// Optionally filter by event category.
pub async fn list_by_tournament(
    pool: &PgPool,
    tournament_id: Uuid,
    category_filter: Option<&str>,
    limit: i64,
    offset: i64,
) -> SqlxResult<Vec<TournamentActivityLogRow>> {
    if let Some(category) = category_filter {
        sqlx::query_as::<_, TournamentActivityLogRow>(
            "SELECT id, tournament_id, event_category, event_action, actor_id, subject_id, event_time, metadata
             FROM tournament_activity_log
             WHERE tournament_id = $1 AND event_category = $2
             ORDER BY event_time DESC
             LIMIT $3 OFFSET $4",
        )
        .bind(tournament_id)
        .bind(category)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, TournamentActivityLogRow>(
            "SELECT id, tournament_id, event_category, event_action, actor_id, subject_id, event_time, metadata
             FROM tournament_activity_log
             WHERE tournament_id = $1
             ORDER BY event_time DESC
             LIMIT $2 OFFSET $3",
        )
        .bind(tournament_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }
}

/// Count activity log entries for a tournament, optionally filtered by category.
pub async fn count_by_tournament(
    pool: &PgPool,
    tournament_id: Uuid,
    category_filter: Option<&str>,
) -> SqlxResult<i64> {
    if let Some(category) = category_filter {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM tournament_activity_log
             WHERE tournament_id = $1 AND event_category = $2",
        )
        .bind(tournament_id)
        .bind(category)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    } else {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM tournament_activity_log
             WHERE tournament_id = $1",
        )
        .bind(tournament_id)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }
}
