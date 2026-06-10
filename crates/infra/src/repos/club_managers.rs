use serde::{Deserialize, Serialize};
use sqlx::Result;
use uuid::Uuid;

use crate::db::Db;
use crate::models::ClubManagerRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClubInfo {
    pub club_id: Uuid,
    pub club_name: String,
}

pub async fn is_club_manager(pool: &Db, user_id: Uuid, club_id: Uuid) -> Result<bool> {
    let result = sqlx::query_scalar!("SELECT is_club_manager($1, $2)", user_id, club_id)
        .fetch_one(pool)
        .await?;

    Ok(result.unwrap_or(false))
}

pub async fn get_manager_clubs(pool: &Db, user_id: Uuid) -> Result<Vec<ClubInfo>> {
    let rows: Vec<_> = sqlx::query!(
        r#"
            SELECT club_id, club_name
            FROM get_manager_clubs($1)
            "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    let clubs = rows
        .into_iter()
        .map(|row| ClubInfo {
            club_id: row.club_id.expect("Club ID should not be null"),
            club_name: row.club_name.expect("Club name should not be null"),
        })
        .collect();

    Ok(clubs)
}

pub async fn list_by_club(pool: &Db, club_id: Uuid) -> Result<Vec<ClubManagerRow>> {
    let rows = sqlx::query_as!(
        ClubManagerRow,
        "
            SELECT cm.*
            FROM club_managers cm
            JOIN users u ON cm.user_id = u.id
            WHERE cm.club_id = $1 
              AND cm.is_active = true
              AND u.role = 'manager'
              AND u.is_active = true
            ORDER BY cm.assigned_at
            ",
        club_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_by_id(pool: &Db, id: Uuid) -> Result<Option<ClubManagerRow>> {
    let row = sqlx::query_as!(
        ClubManagerRow,
        "SELECT * FROM club_managers WHERE id = $1",
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn deactivate(pool: &Db, id: Uuid) -> Result<Option<ClubManagerRow>> {
    let row = sqlx::query_as!(
        ClubManagerRow,
        "
            UPDATE club_managers 
            SET is_active = false, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            ",
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn reactivate(pool: &Db, id: Uuid) -> Result<Option<ClubManagerRow>> {
    let row = sqlx::query_as!(
        ClubManagerRow,
        "
            UPDATE club_managers 
            SET is_active = true, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            ",
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}
