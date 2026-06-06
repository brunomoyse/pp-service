use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::{AttendanceStreakRow, CheckInRow};

/// Record a check-in. Idempotent per (user, tournament): returns the row only
/// when it was newly created, None if the player had already checked in.
pub async fn record_check_in<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    tournament_id: Uuid,
    club_id: Uuid,
) -> SqlxResult<Option<CheckInRow>> {
    sqlx::query_as::<_, CheckInRow>(
        "INSERT INTO check_in (app_user_id, tournament_id, club_id) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (app_user_id, tournament_id) DO NOTHING \
         RETURNING id, app_user_id, tournament_id, club_id, checked_in_at",
    )
    .bind(app_user_id)
    .bind(tournament_id)
    .bind(club_id)
    .fetch_optional(executor)
    .await
}

pub async fn get_streak<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
) -> SqlxResult<Option<AttendanceStreakRow>> {
    sqlx::query_as::<_, AttendanceStreakRow>(
        "SELECT app_user_id, current_streak, longest_streak, last_check_in_at, \
                freezes_available, created_at, updated_at \
         FROM attendance_streak WHERE app_user_id = $1",
    )
    .bind(app_user_id)
    .fetch_optional(executor)
    .await
}

/// Insert or update the user's streak row to the supplied values.
pub async fn upsert_streak<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    current_streak: i32,
    longest_streak: i32,
    last_check_in_at: DateTime<Utc>,
    freezes_available: i32,
) -> SqlxResult<AttendanceStreakRow> {
    sqlx::query_as::<_, AttendanceStreakRow>(
        "INSERT INTO attendance_streak \
            (app_user_id, current_streak, longest_streak, last_check_in_at, freezes_available) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (app_user_id) DO UPDATE SET \
            current_streak = EXCLUDED.current_streak, \
            longest_streak = EXCLUDED.longest_streak, \
            last_check_in_at = EXCLUDED.last_check_in_at, \
            freezes_available = EXCLUDED.freezes_available, \
            updated_at = NOW() \
         RETURNING app_user_id, current_streak, longest_streak, last_check_in_at, \
                   freezes_available, created_at, updated_at",
    )
    .bind(app_user_id)
    .bind(current_streak)
    .bind(longest_streak)
    .bind(last_check_in_at)
    .bind(freezes_available)
    .fetch_one(executor)
    .await
}
