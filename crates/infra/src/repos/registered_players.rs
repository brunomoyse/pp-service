use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::RegisteredPlayerRow;

const COLUMNS: &str = "id, club_id, display_name, app_user_id, created_at, updated_at";

/// Get a single roster entry by id.
pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<RegisteredPlayerRow>> {
    sqlx::query_as::<_, RegisteredPlayerRow>(&format!(
        "SELECT {COLUMNS} FROM registered_player WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// List the full roster for a club, alphabetically by display name.
pub async fn list_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
) -> SqlxResult<Vec<RegisteredPlayerRow>> {
    sqlx::query_as::<_, RegisteredPlayerRow>(&format!(
        "SELECT {COLUMNS} FROM registered_player WHERE club_id = $1 ORDER BY display_name ASC"
    ))
    .bind(club_id)
    .fetch_all(executor)
    .await
}

/// All roster entries an app user is linked to, across every club.
/// The fan-out of these rows is the cross-club profile.
pub async fn list_for_app_user<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
) -> SqlxResult<Vec<RegisteredPlayerRow>> {
    sqlx::query_as::<_, RegisteredPlayerRow>(&format!(
        "SELECT {COLUMNS} FROM registered_player WHERE app_user_id = $1 ORDER BY created_at ASC"
    ))
    .bind(app_user_id)
    .fetch_all(executor)
    .await
}

/// Find an app user's roster entry within a specific club, if any.
pub async fn find_by_club_and_app_user<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    app_user_id: Uuid,
) -> SqlxResult<Option<RegisteredPlayerRow>> {
    sqlx::query_as::<_, RegisteredPlayerRow>(&format!(
        "SELECT {COLUMNS} FROM registered_player WHERE club_id = $1 AND app_user_id = $2"
    ))
    .bind(club_id)
    .bind(app_user_id)
    .fetch_optional(executor)
    .await
}

/// Create a roster entry. `app_user_id` is None for a person who is not (yet) an app user.
pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    display_name: &str,
    app_user_id: Option<Uuid>,
) -> SqlxResult<RegisteredPlayerRow> {
    sqlx::query_as::<_, RegisteredPlayerRow>(&format!(
        "INSERT INTO registered_player (club_id, display_name, app_user_id) \
         VALUES ($1, $2, $3) RETURNING {COLUMNS}"
    ))
    .bind(club_id)
    .bind(display_name)
    .bind(app_user_id)
    .fetch_one(executor)
    .await
}

/// Claim an unclaimed roster entry for an app user (links app_user_id).
/// Only succeeds when the entry exists and is not already claimed; returns the
/// updated row, or None if it was missing or already claimed.
pub async fn claim<'e>(
    executor: impl PgExecutor<'e>,
    registered_player_id: Uuid,
    app_user_id: Uuid,
) -> SqlxResult<Option<RegisteredPlayerRow>> {
    sqlx::query_as::<_, RegisteredPlayerRow>(&format!(
        "UPDATE registered_player \
         SET app_user_id = $2, updated_at = NOW() \
         WHERE id = $1 AND app_user_id IS NULL \
         RETURNING {COLUMNS}"
    ))
    .bind(registered_player_id)
    .bind(app_user_id)
    .fetch_optional(executor)
    .await
}
