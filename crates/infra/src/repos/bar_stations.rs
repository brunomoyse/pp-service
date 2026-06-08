use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::BarStationRow;

const COLUMNS: &str = "id, club_id, name, is_active, created_at, updated_at";

/// Get a single bar station by id.
pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<BarStationRow>> {
    sqlx::query_as::<_, BarStationRow>(&format!("SELECT {COLUMNS} FROM bar_station WHERE id = $1"))
        .bind(id)
        .fetch_optional(executor)
        .await
}

/// List the active bar stations for a club.
pub async fn list_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
) -> SqlxResult<Vec<BarStationRow>> {
    sqlx::query_as::<_, BarStationRow>(&format!(
        "SELECT {COLUMNS} FROM bar_station WHERE club_id = $1 ORDER BY name ASC"
    ))
    .bind(club_id)
    .fetch_all(executor)
    .await
}

/// Create a bar station for a club.
pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    name: &str,
) -> SqlxResult<BarStationRow> {
    sqlx::query_as::<_, BarStationRow>(&format!(
        "INSERT INTO bar_station (club_id, name) VALUES ($1, $2) RETURNING {COLUMNS}"
    ))
    .bind(club_id)
    .bind(name)
    .fetch_one(executor)
    .await
}
