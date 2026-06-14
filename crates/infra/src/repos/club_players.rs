use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::ClubPlayerRow;

const COLUMNS: &str = "id, club_id, display_name, first_name, last_name, app_user_id, is_active, created_at, updated_at";

/// Get a single roster entry by id.
pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<ClubPlayerRow>> {
    sqlx::query_as::<_, ClubPlayerRow>(&format!("SELECT {COLUMNS} FROM club_player WHERE id = $1"))
        .bind(id)
        .fetch_optional(executor)
        .await
}

/// List the active roster for a club, alphabetically by display name.
/// Archived entries (`is_active = false`) are excluded from the roster view;
/// they remain referenced by historical registrations/results.
pub async fn list_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
) -> SqlxResult<Vec<ClubPlayerRow>> {
    sqlx::query_as::<_, ClubPlayerRow>(&format!(
        "SELECT {COLUMNS} FROM club_player \
         WHERE club_id = $1 AND is_active = true \
         ORDER BY COALESCE(NULLIF(last_name, ''), display_name) ASC, first_name ASC"
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
) -> SqlxResult<Vec<ClubPlayerRow>> {
    sqlx::query_as::<_, ClubPlayerRow>(&format!(
        "SELECT {COLUMNS} FROM club_player WHERE app_user_id = $1 ORDER BY created_at ASC"
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
) -> SqlxResult<Option<ClubPlayerRow>> {
    sqlx::query_as::<_, ClubPlayerRow>(&format!(
        "SELECT {COLUMNS} FROM club_player WHERE club_id = $1 AND app_user_id = $2"
    ))
    .bind(club_id)
    .bind(app_user_id)
    .fetch_optional(executor)
    .await
}

/// Create a roster entry. `app_user_id` is None for a person who is not (yet) an
/// app user. `first_name`/`last_name` are None for bulk/single-field imports
/// that only carry a display name.
pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    display_name: &str,
    first_name: Option<&str>,
    last_name: Option<&str>,
    app_user_id: Option<Uuid>,
) -> SqlxResult<ClubPlayerRow> {
    sqlx::query_as::<_, ClubPlayerRow>(&format!(
        "INSERT INTO club_player (club_id, display_name, first_name, last_name, app_user_id) \
         VALUES ($1, $2, $3, $4, $5) RETURNING {COLUMNS}"
    ))
    .bind(club_id)
    .bind(display_name)
    .bind(first_name)
    .bind(last_name)
    .bind(app_user_id)
    .fetch_one(executor)
    .await
}

/// Claim an unclaimed roster entry for an app user (links app_user_id).
/// Only succeeds when the entry exists and is not already claimed; returns the
/// updated row, or None if it was missing or already claimed.
pub async fn claim<'e>(
    executor: impl PgExecutor<'e>,
    club_player_id: Uuid,
    app_user_id: Uuid,
) -> SqlxResult<Option<ClubPlayerRow>> {
    sqlx::query_as::<_, ClubPlayerRow>(&format!(
        "UPDATE club_player \
         SET app_user_id = $2, updated_at = NOW() \
         WHERE id = $1 AND app_user_id IS NULL \
         RETURNING {COLUMNS}"
    ))
    .bind(club_player_id)
    .bind(app_user_id)
    .fetch_optional(executor)
    .await
}

/// Rename a roster entry. Scoped to its club. Returns the updated row, or None
/// if no entry with that id exists in the club.
pub async fn update_display_name<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    club_id: Uuid,
    display_name: &str,
) -> SqlxResult<Option<ClubPlayerRow>> {
    sqlx::query_as::<_, ClubPlayerRow>(&format!(
        "UPDATE club_player \
         SET display_name = $3, updated_at = NOW() \
         WHERE id = $1 AND club_id = $2 RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(club_id)
    .bind(display_name)
    .fetch_optional(executor)
    .await
}

/// Update a roster entry's structured name (first + last), keeping `display_name`
/// in sync as "first last". Scoped to its club. Returns the updated row, or None
/// if no entry with that id exists in the club.
pub async fn update_name<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    club_id: Uuid,
    first_name: &str,
    last_name: &str,
    display_name: &str,
) -> SqlxResult<Option<ClubPlayerRow>> {
    sqlx::query_as::<_, ClubPlayerRow>(&format!(
        "UPDATE club_player \
         SET first_name = $3, last_name = $4, display_name = $5, updated_at = NOW() \
         WHERE id = $1 AND club_id = $2 RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(club_id)
    .bind(first_name)
    .bind(last_name)
    .bind(display_name)
    .fetch_optional(executor)
    .await
}

/// Anonymise an *unclaimed* roster entry: scrub its name to a neutral
/// placeholder and deactivate it, while keeping the row so historical
/// registrations/results/entries stay intact. Only affects entries with no
/// linked app account (`app_user_id IS NULL`); claimed entries are owned by the
/// app user and managed via account deletion instead. Returns the updated row,
/// or None if the entry is missing, in another club, or claimed.
pub async fn anonymize<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    club_id: Uuid,
    placeholder: &str,
) -> SqlxResult<Option<ClubPlayerRow>> {
    sqlx::query_as::<_, ClubPlayerRow>(&format!(
        "UPDATE club_player \
         SET display_name = $3, first_name = NULL, last_name = NULL, \
             is_active = false, updated_at = NOW() \
         WHERE id = $1 AND club_id = $2 AND app_user_id IS NULL \
         RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(club_id)
    .bind(placeholder)
    .fetch_optional(executor)
    .await
}

/// Archive or restore a roster entry (soft delete). Scoped to its club. Returns
/// the updated row, or None if no entry with that id exists in the club.
pub async fn set_active<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    club_id: Uuid,
    is_active: bool,
) -> SqlxResult<Option<ClubPlayerRow>> {
    sqlx::query_as::<_, ClubPlayerRow>(&format!(
        "UPDATE club_player \
         SET is_active = $3, updated_at = NOW() \
         WHERE id = $1 AND club_id = $2 RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(club_id)
    .bind(is_active)
    .fetch_optional(executor)
    .await
}
