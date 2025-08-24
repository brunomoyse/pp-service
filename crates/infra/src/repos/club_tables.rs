use crate::{
    db::Db,
    models::{ClubTableRow, TournamentTableAssignmentRow},
};
use sqlx::Result as SqlxResult;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CreateClubTable {
    pub club_id: Uuid,
    pub table_number: i32,
    pub max_seats: i32,
    pub table_name: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateClubTable {
    pub max_seats: Option<i32>,
    pub is_active: Option<bool>,
    pub table_name: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClubTableRepo {
    pool: Db,
}

impl ClubTableRepo {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }

    /// Create a new club table
    pub async fn create(&self, data: CreateClubTable) -> SqlxResult<ClubTableRow> {
        sqlx::query_as::<_, ClubTableRow>(
            r#"
            INSERT INTO club_tables (club_id, table_number, max_seats, table_name, location)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, club_id, table_number, max_seats, table_name, location, is_active, created_at, updated_at
            "#
        )
        .bind(data.club_id)
        .bind(data.table_number)
        .bind(data.max_seats)
        .bind(data.table_name)
        .bind(data.location)
        .fetch_one(&self.pool)
        .await
    }

    /// Get a club table by ID
    pub async fn get_by_id(&self, id: Uuid) -> SqlxResult<Option<ClubTableRow>> {
        sqlx::query_as::<_, ClubTableRow>(
            r#"
            SELECT id, club_id, table_number, max_seats, table_name, location, is_active, created_at, updated_at
            FROM club_tables
            WHERE id = $1
            "#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Get all tables for a club
    pub async fn get_by_club(&self, club_id: Uuid) -> SqlxResult<Vec<ClubTableRow>> {
        sqlx::query_as::<_, ClubTableRow>(
            r#"
            SELECT id, club_id, table_number, max_seats, table_name, location, is_active, created_at, updated_at
            FROM club_tables
            WHERE club_id = $1
            ORDER BY table_number ASC
            "#
        )
        .bind(club_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Get only active tables for a club
    pub async fn get_active_by_club(&self, club_id: Uuid) -> SqlxResult<Vec<ClubTableRow>> {
        sqlx::query_as::<_, ClubTableRow>(
            r#"
            SELECT id, club_id, table_number, max_seats, table_name, location, is_active, created_at, updated_at
            FROM club_tables
            WHERE club_id = $1 AND is_active = true
            ORDER BY table_number ASC
            "#
        )
        .bind(club_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Get available tables for a club (not assigned to active tournaments)
    pub async fn get_available_by_club(&self, club_id: Uuid) -> SqlxResult<Vec<ClubTableRow>> {
        sqlx::query_as::<_, ClubTableRow>(
            r#"
            SELECT ct.id, ct.club_id, ct.table_number, ct.max_seats, ct.table_name, ct.location, ct.is_active, ct.created_at, ct.updated_at
            FROM club_tables ct
            LEFT JOIN tournament_table_assignments tta ON ct.id = tta.club_table_id 
                AND tta.is_active = true
                AND EXISTS (
                    SELECT 1 FROM tournaments t 
                    WHERE t.id = tta.tournament_id 
                    AND t.live_status IN ('not_started', 'late_registration', 'in_progress', 'break')
                )
            WHERE ct.club_id = $1
                AND ct.is_active = true
                AND tta.id IS NULL
            ORDER BY ct.table_number ASC
            "#
        )
        .bind(club_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Update a club table
    pub async fn update(
        &self,
        id: Uuid,
        data: UpdateClubTable,
    ) -> SqlxResult<Option<ClubTableRow>> {
        sqlx::query_as::<_, ClubTableRow>(
            r#"
            UPDATE club_tables
            SET max_seats = COALESCE($2, max_seats),
                is_active = COALESCE($3, is_active),
                table_name = COALESCE($4, table_name),
                location = COALESCE($5, location),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, club_id, table_number, max_seats, table_name, location, is_active, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(data.max_seats)
        .bind(data.is_active)
        .bind(data.table_name)
        .bind(data.location)
        .fetch_optional(&self.pool)
        .await
    }

    /// Delete a club table
    pub async fn delete(&self, id: Uuid) -> SqlxResult<bool> {
        let result = sqlx::query("DELETE FROM club_tables WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Assign a club table to a tournament
    pub async fn assign_to_tournament(
        &self,
        tournament_id: Uuid,
        club_table_id: Uuid,
    ) -> SqlxResult<TournamentTableAssignmentRow> {
        sqlx::query_as::<_, TournamentTableAssignmentRow>(
            r#"
            INSERT INTO tournament_table_assignments (tournament_id, club_table_id)
            VALUES ($1, $2)
            ON CONFLICT (tournament_id, club_table_id) DO UPDATE SET
                is_active = true,
                assigned_at = NOW(),
                deactivated_at = NULL,
                updated_at = NOW()
            RETURNING id, tournament_id, club_table_id, is_active, assigned_at, deactivated_at, created_at, updated_at
            "#
        )
        .bind(tournament_id)
        .bind(club_table_id)
        .fetch_one(&self.pool)
        .await
    }

    /// Remove table assignment from tournament
    pub async fn unassign_from_tournament(
        &self,
        tournament_id: Uuid,
        club_table_id: Uuid,
    ) -> SqlxResult<bool> {
        let result = sqlx::query(
            r#"
            UPDATE tournament_table_assignments 
            SET is_active = false, deactivated_at = NOW(), updated_at = NOW()
            WHERE tournament_id = $1 AND club_table_id = $2
            "#,
        )
        .bind(tournament_id)
        .bind(club_table_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get assigned tables for a tournament
    pub async fn get_assigned_to_tournament(
        &self,
        tournament_id: Uuid,
    ) -> SqlxResult<Vec<ClubTableRow>> {
        sqlx::query_as::<_, ClubTableRow>(
            r#"
            SELECT ct.id, ct.club_id, ct.table_number, ct.max_seats, ct.table_name, ct.location, ct.is_active, ct.created_at, ct.updated_at
            FROM club_tables ct
            INNER JOIN tournament_table_assignments tta ON ct.id = tta.club_table_id
            WHERE tta.tournament_id = $1 AND tta.is_active = true
            ORDER BY ct.table_number ASC
            "#
        )
        .bind(tournament_id)
        .fetch_all(&self.pool)
        .await
    }
}
