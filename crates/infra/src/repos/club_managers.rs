use serde::{Deserialize, Serialize};
use sqlx::Result;
use uuid::Uuid;

use crate::db::Db;
use crate::models::ClubManagerRow;

pub struct ClubManagerRepo {
    db: Db,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateClubManager {
    pub club_id: Uuid,
    pub user_id: Uuid,
    pub assigned_by: Option<Uuid>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClubInfo {
    pub club_id: Uuid,
    pub club_name: String,
}

impl ClubManagerRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Assign a manager to a club
    pub async fn create(&self, create_club_manager: CreateClubManager) -> Result<ClubManagerRow> {
        let row = sqlx::query_as!(
            ClubManagerRow,
            r#"
            INSERT INTO club_managers (club_id, user_id, assigned_by, notes)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
            create_club_manager.club_id,
            create_club_manager.user_id,
            create_club_manager.assigned_by,
            create_club_manager.notes
        )
        .fetch_one(&self.db)
        .await?;

        Ok(row)
    }

    /// Check if a user is an active manager of a specific club
    pub async fn is_club_manager(&self, user_id: Uuid, club_id: Uuid) -> Result<bool> {
        let result = sqlx::query_scalar!("SELECT is_club_manager($1, $2)", user_id, club_id)
            .fetch_one(&self.db)
            .await?;

        Ok(result.unwrap_or(false))
    }

    /// Get all clubs a manager can manage
    pub async fn get_manager_clubs(&self, user_id: Uuid) -> Result<Vec<ClubInfo>> {
        let rows = sqlx::query!(
            r#"
            SELECT club_id, club_name
            FROM get_manager_clubs($1)
            "#,
            user_id
        )
        .fetch_all(&self.db)
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

    /// Get all active managers for a specific club
    pub async fn get_club_managers(&self, club_id: Uuid) -> Result<Vec<ClubManagerRow>> {
        let rows = sqlx::query_as!(
            ClubManagerRow,
            r#"
            SELECT cm.*
            FROM club_managers cm
            JOIN users u ON cm.user_id = u.id
            WHERE cm.club_id = $1 
              AND cm.is_active = true
              AND u.role = 'manager'
              AND u.is_active = true
            ORDER BY cm.assigned_at
            "#,
            club_id
        )
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }

    /// Get a specific club manager assignment
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<ClubManagerRow>> {
        let row = sqlx::query_as!(
            ClubManagerRow,
            "SELECT * FROM club_managers WHERE id = $1",
            id
        )
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    /// Get club manager assignment by user and club
    pub async fn get_by_user_and_club(
        &self,
        user_id: Uuid,
        club_id: Uuid,
    ) -> Result<Option<ClubManagerRow>> {
        let row = sqlx::query_as!(
            ClubManagerRow,
            r#"
            SELECT * FROM club_managers 
            WHERE user_id = $1 AND club_id = $2 AND is_active = true
            "#,
            user_id,
            club_id
        )
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    /// Deactivate a club manager assignment
    pub async fn deactivate(&self, id: Uuid) -> Result<Option<ClubManagerRow>> {
        let row = sqlx::query_as!(
            ClubManagerRow,
            r#"
            UPDATE club_managers 
            SET is_active = false, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
            id
        )
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    /// Reactivate a club manager assignment
    pub async fn reactivate(&self, id: Uuid) -> Result<Option<ClubManagerRow>> {
        let row = sqlx::query_as!(
            ClubManagerRow,
            r#"
            UPDATE club_managers 
            SET is_active = true, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
            id
        )
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    /// Update notes for a club manager assignment
    pub async fn update_notes(
        &self,
        id: Uuid,
        notes: Option<String>,
    ) -> Result<Option<ClubManagerRow>> {
        let row = sqlx::query_as!(
            ClubManagerRow,
            r#"
            UPDATE club_managers 
            SET notes = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
            id,
            notes
        )
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }
}
