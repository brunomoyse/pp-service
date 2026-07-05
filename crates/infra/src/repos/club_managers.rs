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

/// A club-manager assignment joined with the manager's user record, for team lists.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ClubManagerWithUser {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub assigned_at: chrono::DateTime<chrono::Utc>,
}

/// Active assignments joined with user info. Unlike `list_by_club` this does not
/// filter on role, so club-managing admins appear in the team list too.
pub async fn list_by_club_with_users(pool: &Db, club_id: Uuid) -> Result<Vec<ClubManagerWithUser>> {
    sqlx::query_as::<_, ClubManagerWithUser>(
        "
        SELECT cm.id, cm.user_id, u.email, u.first_name, u.last_name, cm.assigned_at
        FROM club_managers cm
        JOIN users u ON u.id = cm.user_id
        WHERE cm.club_id = $1 AND cm.is_active = true AND u.is_active = true
        ORDER BY cm.assigned_at
        ",
    )
    .bind(club_id)
    .fetch_all(pool)
    .await
}

/// Assign a user as manager of a club; a no-op returning the existing row if an
/// active assignment already exists (deactivated history rows are left intact).
pub async fn create_or_reactivate(
    pool: &Db,
    club_id: Uuid,
    user_id: Uuid,
    assigned_by: Uuid,
) -> Result<ClubManagerRow> {
    sqlx::query_as::<_, ClubManagerRow>(
        "
        INSERT INTO club_managers (club_id, user_id, assigned_by)
        VALUES ($1, $2, $3)
        ON CONFLICT (club_id, user_id) WHERE is_active = true
        DO UPDATE SET updated_at = now()
        RETURNING *
        ",
    )
    .bind(club_id)
    .bind(user_id)
    .bind(assigned_by)
    .fetch_one(pool)
    .await
}

pub async fn count_active_by_club(pool: &Db, club_id: Uuid) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM club_managers WHERE club_id = $1 AND is_active = true",
    )
    .bind(club_id)
    .fetch_one(pool)
    .await
}
