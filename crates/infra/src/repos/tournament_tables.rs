use crate::{db::Db, models::TournamentTableRow};
use sqlx::Result as SqlxResult;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CreateTournamentTable {
    pub tournament_id: Uuid,
    pub table_number: i32,
    pub max_seats: i32,
    pub table_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateTournamentTable {
    pub max_seats: Option<i32>,
    pub is_active: Option<bool>,
    pub table_name: Option<String>,
}

#[derive(Clone)]
pub struct TournamentTableRepo {
    pool: Db,
}

impl TournamentTableRepo {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }

    /// Create a new tournament table
    pub async fn create(&self, data: CreateTournamentTable) -> SqlxResult<TournamentTableRow> {
        sqlx::query_as::<_, TournamentTableRow>(
            r#"
            INSERT INTO tournament_tables (tournament_id, table_number, max_seats, table_name)
            VALUES ($1, $2, $3, $4)
            RETURNING id, tournament_id, table_number, max_seats, is_active, table_name, created_at, updated_at
            "#
        )
        .bind(data.tournament_id)
        .bind(data.table_number)
        .bind(data.max_seats)
        .bind(data.table_name)
        .fetch_one(&self.pool)
        .await
    }

    /// Get a tournament table by ID
    pub async fn get_by_id(&self, id: Uuid) -> SqlxResult<Option<TournamentTableRow>> {
        sqlx::query_as::<_, TournamentTableRow>(
            r#"
            SELECT id, tournament_id, table_number, max_seats, is_active, table_name, created_at, updated_at
            FROM tournament_tables
            WHERE id = $1
            "#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Get all tables for a tournament
    pub async fn get_by_tournament(&self, tournament_id: Uuid) -> SqlxResult<Vec<TournamentTableRow>> {
        sqlx::query_as::<_, TournamentTableRow>(
            r#"
            SELECT id, tournament_id, table_number, max_seats, is_active, table_name, created_at, updated_at
            FROM tournament_tables
            WHERE tournament_id = $1
            ORDER BY table_number ASC
            "#
        )
        .bind(tournament_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Get only active tables for a tournament
    pub async fn get_active_by_tournament(&self, tournament_id: Uuid) -> SqlxResult<Vec<TournamentTableRow>> {
        sqlx::query_as::<_, TournamentTableRow>(
            r#"
            SELECT id, tournament_id, table_number, max_seats, is_active, table_name, created_at, updated_at
            FROM tournament_tables
            WHERE tournament_id = $1 AND is_active = true
            ORDER BY table_number ASC
            "#
        )
        .bind(tournament_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Update a tournament table
    pub async fn update(&self, id: Uuid, data: UpdateTournamentTable) -> SqlxResult<Option<TournamentTableRow>> {
        sqlx::query_as::<_, TournamentTableRow>(
            r#"
            UPDATE tournament_tables
            SET max_seats = COALESCE($2, max_seats),
                is_active = COALESCE($3, is_active),
                table_name = COALESCE($4, table_name),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, tournament_id, table_number, max_seats, is_active, table_name, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(data.max_seats)
        .bind(data.is_active)
        .bind(data.table_name)
        .fetch_optional(&self.pool)
        .await
    }

    /// Delete a tournament table (and all its seat assignments)
    pub async fn delete(&self, id: Uuid) -> SqlxResult<bool> {
        let result = sqlx::query(
            "DELETE FROM tournament_tables WHERE id = $1"
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Set table as inactive (soft delete)
    pub async fn deactivate(&self, id: Uuid) -> SqlxResult<bool> {
        let result = sqlx::query(
            r#"
            UPDATE tournament_tables 
            SET is_active = false, updated_at = NOW()
            WHERE id = $1
            "#
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get table by tournament and table number
    pub async fn get_by_tournament_and_number(
        &self,
        tournament_id: Uuid,
        table_number: i32,
    ) -> SqlxResult<Option<TournamentTableRow>> {
        sqlx::query_as::<_, TournamentTableRow>(
            r#"
            SELECT id, tournament_id, table_number, max_seats, is_active, table_name, created_at, updated_at
            FROM tournament_tables
            WHERE tournament_id = $1 AND table_number = $2
            "#
        )
        .bind(tournament_id)
        .bind(table_number)
        .fetch_optional(&self.pool)
        .await
    }

    /// Count active tables for a tournament
    pub async fn count_active_tables(&self, tournament_id: Uuid) -> SqlxResult<i64> {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM tournament_tables WHERE tournament_id = $1 AND is_active = true"
        )
        .bind(tournament_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(result.0)
    }
}