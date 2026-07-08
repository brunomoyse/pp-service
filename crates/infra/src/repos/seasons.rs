use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::{SeasonChampionRow, SeasonPassRow, SeasonRow};

const SEASON_COLS: &str = "id, club_id, name, starts_at, ends_at, created_at";

pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    name: &str,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
) -> SqlxResult<SeasonRow> {
    sqlx::query_as::<_, SeasonRow>(
        "INSERT INTO season (club_id, name, starts_at, ends_at) \
         VALUES ($1, $2, $3, $4) \
         RETURNING id, club_id, name, starts_at, ends_at, created_at",
    )
    .bind(club_id)
    .bind(name)
    .bind(starts_at)
    .bind(ends_at)
    .fetch_one(executor)
    .await
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<SeasonRow>> {
    sqlx::query_as::<_, SeasonRow>(&format!("SELECT {SEASON_COLS} FROM season WHERE id = $1"))
        .bind(id)
        .fetch_optional(executor)
        .await
}

pub async fn list_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
) -> SqlxResult<Vec<SeasonRow>> {
    sqlx::query_as::<_, SeasonRow>(&format!(
        "SELECT {SEASON_COLS} FROM season WHERE club_id = $1 ORDER BY starts_at DESC"
    ))
    .bind(club_id)
    .fetch_all(executor)
    .await
}

/// The club's currently-running season (now within its window), if any.
pub async fn current_for_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    now: DateTime<Utc>,
) -> SqlxResult<Option<SeasonRow>> {
    sqlx::query_as::<_, SeasonRow>(&format!(
        "SELECT {SEASON_COLS} FROM season \
         WHERE club_id = $1 AND starts_at <= $2 AND ends_at > $2 \
         ORDER BY starts_at DESC LIMIT 1"
    ))
    .bind(club_id)
    .bind(now)
    .fetch_all(executor)
    .await
    .map(|mut rows| rows.drain(..).next())
}

/// Seasons that have already ended, newest first (for the Hall of Fame).
pub async fn list_finished_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    now: DateTime<Utc>,
) -> SqlxResult<Vec<SeasonRow>> {
    sqlx::query_as::<_, SeasonRow>(&format!(
        "SELECT {SEASON_COLS} FROM season \
         WHERE club_id = $1 AND ends_at <= $2 ORDER BY ends_at DESC"
    ))
    .bind(club_id)
    .bind(now)
    .fetch_all(executor)
    .await
}

// ---- Season pass ----------------------------------------------------------

pub async fn get_pass<'e>(
    executor: impl PgExecutor<'e>,
    season_id: Uuid,
    app_user_id: Uuid,
) -> SqlxResult<Option<SeasonPassRow>> {
    sqlx::query_as::<_, SeasonPassRow>(
        "SELECT id, season_id, app_user_id, created_at, updated_at \
         FROM season_pass WHERE season_id = $1 AND app_user_id = $2",
    )
    .bind(season_id)
    .bind(app_user_id)
    .fetch_optional(executor)
    .await
}

/// Create the pass row if missing.
pub async fn ensure_pass<'e>(
    executor: impl PgExecutor<'e>,
    season_id: Uuid,
    app_user_id: Uuid,
) -> SqlxResult<SeasonPassRow> {
    sqlx::query_as::<_, SeasonPassRow>(
        "INSERT INTO season_pass (season_id, app_user_id) VALUES ($1, $2) \
         ON CONFLICT (season_id, app_user_id) DO UPDATE SET season_id = EXCLUDED.season_id \
         RETURNING id, season_id, app_user_id, created_at, updated_at",
    )
    .bind(season_id)
    .bind(app_user_id)
    .fetch_one(executor)
    .await
}

/// Count a player's check-ins for a club within a window — the season-pass
/// attendance XP base.
pub async fn check_in_count_in_window<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    club_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> SqlxResult<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM check_in \
         WHERE app_user_id = $1 AND club_id = $2 AND checked_in_at >= $3 AND checked_in_at < $4",
    )
    .bind(app_user_id)
    .bind(club_id)
    .bind(start)
    .bind(end)
    .fetch_one(executor)
    .await?;
    Ok(row.0)
}

/// The most-present player for a club within a window — the season champion.
pub async fn champion_for_window<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> SqlxResult<Option<SeasonChampionRow>> {
    sqlx::query_as::<_, SeasonChampionRow>(
        "SELECT ci.app_user_id AS app_user_id, \
                COALESCE(u.username, u.first_name) AS champion_name, \
                COUNT(*) AS events \
         FROM check_in ci JOIN users u ON u.id = ci.app_user_id \
         WHERE ci.club_id = $1 AND ci.checked_in_at >= $2 AND ci.checked_in_at < $3 \
         GROUP BY ci.app_user_id, u.username, u.first_name \
         ORDER BY events DESC, champion_name ASC LIMIT 1",
    )
    .bind(club_id)
    .bind(start)
    .bind(end)
    .fetch_optional(executor)
    .await
}
