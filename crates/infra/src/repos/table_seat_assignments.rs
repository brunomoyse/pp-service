use crate::{
    db::Db,
    models::{TableSeatAssignmentRow, UserRow},
};
use chrono::{DateTime, Utc};
use sqlx::Result as SqlxResult;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CreateSeatAssignment {
    pub tournament_id: Uuid,
    pub table_id: Uuid,
    pub user_id: Uuid,
    pub seat_number: i32,
    pub stack_size: Option<i32>,
    pub assigned_by: Option<Uuid>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateSeatAssignment {
    pub stack_size: Option<i32>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SeatAssignmentWithPlayer {
    pub assignment: TableSeatAssignmentRow,
    pub player: UserRow,
}

#[derive(Debug, Clone)]
pub struct SeatAssignmentFilter {
    pub tournament_id: Option<Uuid>,
    pub table_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub is_current: Option<bool>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct TableSeatAssignmentRepo {
    pool: Db,
}

impl TableSeatAssignmentRepo {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }

    /// Create a new seat assignment
    pub async fn create(&self, data: CreateSeatAssignment) -> SqlxResult<TableSeatAssignmentRow> {
        sqlx::query_as::<_, TableSeatAssignmentRow>(
            r#"
            INSERT INTO table_seat_assignments (
                tournament_id, table_id, user_id, seat_number, 
                stack_size, assigned_by, notes
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, tournament_id, table_id, user_id, seat_number, stack_size, 
                     is_current, assigned_at, unassigned_at, assigned_by, notes, created_at, updated_at
            "#
        )
        .bind(data.tournament_id)
        .bind(data.table_id)
        .bind(data.user_id)
        .bind(data.seat_number)
        .bind(data.stack_size)
        .bind(data.assigned_by)
        .bind(data.notes)
        .fetch_one(&self.pool)
        .await
    }

    /// Get seat assignment by ID
    pub async fn get_by_id(&self, id: Uuid) -> SqlxResult<Option<TableSeatAssignmentRow>> {
        sqlx::query_as::<_, TableSeatAssignmentRow>(
            r#"
            SELECT id, tournament_id, table_id, user_id, seat_number, stack_size, 
                   is_current, assigned_at, unassigned_at, assigned_by, notes, created_at, updated_at
            FROM table_seat_assignments
            WHERE id = $1
            "#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Get current seat assignment for a user in a tournament
    pub async fn get_current_for_user(
        &self,
        tournament_id: Uuid,
        user_id: Uuid,
    ) -> SqlxResult<Option<TableSeatAssignmentRow>> {
        sqlx::query_as::<_, TableSeatAssignmentRow>(
            r#"
            SELECT id, tournament_id, table_id, user_id, seat_number, stack_size, 
                   is_current, assigned_at, unassigned_at, assigned_by, notes, created_at, updated_at
            FROM table_seat_assignments
            WHERE tournament_id = $1 AND user_id = $2 AND is_current = true
            "#
        )
        .bind(tournament_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Get all current seat assignments for a table
    pub async fn get_current_for_table(
        &self,
        table_id: Uuid,
    ) -> SqlxResult<Vec<TableSeatAssignmentRow>> {
        sqlx::query_as::<_, TableSeatAssignmentRow>(
            r#"
            SELECT id, tournament_id, table_id, user_id, seat_number, stack_size, 
                   is_current, assigned_at, unassigned_at, assigned_by, notes, created_at, updated_at
            FROM table_seat_assignments
            WHERE table_id = $1 AND is_current = true
            ORDER BY seat_number ASC
            "#
        )
        .bind(table_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Get all current seat assignments for a tournament
    pub async fn get_current_for_tournament(
        &self,
        tournament_id: Uuid,
    ) -> SqlxResult<Vec<TableSeatAssignmentRow>> {
        sqlx::query_as::<_, TableSeatAssignmentRow>(
            r#"
            SELECT id, tournament_id, table_id, user_id, seat_number, stack_size, 
                   is_current, assigned_at, unassigned_at, assigned_by, notes, created_at, updated_at
            FROM table_seat_assignments
            WHERE tournament_id = $1 AND is_current = true
            ORDER BY table_id, seat_number ASC
            "#
        )
        .bind(tournament_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Get current seat assignments with player information for a table
    pub async fn get_current_with_players_for_table(
        &self,
        table_id: Uuid,
    ) -> SqlxResult<Vec<SeatAssignmentWithPlayer>> {
        #[derive(sqlx::FromRow)]
        struct JoinedRow {
            // Assignment fields
            id: Uuid,
            tournament_id: Uuid,
            table_id: Uuid,
            user_id: Uuid,
            seat_number: i32,
            stack_size: Option<i32>,
            is_current: bool,
            assigned_at: DateTime<Utc>,
            // User fields
            email: String,
            username: Option<String>,
            first_name: String,
            last_name: Option<String>,
            phone: Option<String>,
            is_active: bool,
            role: Option<String>,
            user_created_at: DateTime<Utc>,
            user_updated_at: DateTime<Utc>,
        }

        let rows = sqlx::query_as::<_, JoinedRow>(
            r#"
            SELECT 
                tsa.id, tsa.tournament_id, tsa.table_id, tsa.user_id, tsa.seat_number, 
                tsa.stack_size, tsa.is_current, tsa.assigned_at, tsa.unassigned_at, 
                tsa.assigned_by, tsa.notes, tsa.created_at, tsa.updated_at,
                u.email, u.username, u.first_name, u.last_name, u.phone, u.is_active, u.role,
                u.created_at as user_created_at, u.updated_at as user_updated_at
            FROM table_seat_assignments tsa
            JOIN users u ON tsa.user_id = u.id
            WHERE tsa.table_id = $1 AND tsa.is_current = true
            ORDER BY tsa.seat_number ASC
            "#,
        )
        .bind(table_id)
        .fetch_all(&self.pool)
        .await?;

        let results = rows
            .into_iter()
            .map(|row| SeatAssignmentWithPlayer {
                assignment: TableSeatAssignmentRow {
                    id: row.id,
                    tournament_id: row.tournament_id,
                    table_id: row.table_id,
                    user_id: row.user_id,
                    seat_number: row.seat_number,
                    stack_size: row.stack_size,
                    is_current: row.is_current,
                    assigned_at: row.assigned_at,
                },
                player: UserRow {
                    id: row.user_id,
                    email: row.email,
                    username: row.username,
                    first_name: row.first_name,
                    last_name: row.last_name,
                    phone: row.phone,
                    is_active: row.is_active,
                    role: row.role,
                    created_at: row.user_created_at,
                    updated_at: row.user_updated_at,
                },
            })
            .collect();

        Ok(results)
    }

    /// Get seat assignment history with filter
    pub async fn get_history(
        &self,
        filter: SeatAssignmentFilter,
        limit: Option<i64>,
    ) -> SqlxResult<Vec<TableSeatAssignmentRow>> {
        let limit = limit.unwrap_or(100).min(1000); // Cap at 1000 for safety

        sqlx::query_as::<_, TableSeatAssignmentRow>(
            r#"
            SELECT id, tournament_id, table_id, user_id, seat_number, stack_size, 
                   is_current, assigned_at, unassigned_at, assigned_by, notes, created_at, updated_at
            FROM table_seat_assignments
            WHERE ($1::uuid IS NULL OR tournament_id = $1)
              AND ($2::uuid IS NULL OR table_id = $2)
              AND ($3::uuid IS NULL OR user_id = $3)
              AND ($4::boolean IS NULL OR is_current = $4)
              AND ($5::timestamptz IS NULL OR assigned_at >= $5)
              AND ($6::timestamptz IS NULL OR assigned_at <= $6)
            ORDER BY assigned_at DESC
            LIMIT $7
            "#
        )
        .bind(filter.tournament_id)
        .bind(filter.table_id)
        .bind(filter.user_id)
        .bind(filter.is_current)
        .bind(filter.from_date)
        .bind(filter.to_date)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Update seat assignment (typically for stack size updates)
    pub async fn update(
        &self,
        id: Uuid,
        data: UpdateSeatAssignment,
    ) -> SqlxResult<Option<TableSeatAssignmentRow>> {
        sqlx::query_as::<_, TableSeatAssignmentRow>(
            r#"
            UPDATE table_seat_assignments
            SET stack_size = COALESCE($2, stack_size),
                notes = COALESCE($3, notes),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, tournament_id, table_id, user_id, seat_number, stack_size, 
                     is_current, assigned_at, unassigned_at, assigned_by, notes, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(data.stack_size)
        .bind(data.notes)
        .fetch_optional(&self.pool)
        .await
    }

    /// Unassign a player (mark as not current)
    pub async fn unassign(
        &self,
        id: Uuid,
        unassigned_by: Option<Uuid>,
    ) -> SqlxResult<Option<TableSeatAssignmentRow>> {
        sqlx::query_as::<_, TableSeatAssignmentRow>(
            r#"
            UPDATE table_seat_assignments
            SET is_current = false,
                unassigned_at = NOW(),
                assigned_by = COALESCE($2, assigned_by),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, tournament_id, table_id, user_id, seat_number, stack_size, 
                     is_current, assigned_at, unassigned_at, assigned_by, notes, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(unassigned_by)
        .fetch_optional(&self.pool)
        .await
    }

    /// Move a player to a new seat (creates new assignment and unassigns old one)
    pub async fn move_player(
        &self,
        tournament_id: Uuid,
        user_id: Uuid,
        new_table_id: Uuid,
        new_seat_number: i32,
        moved_by: Option<Uuid>,
        notes: Option<String>,
    ) -> SqlxResult<TableSeatAssignmentRow> {
        // Start a transaction to ensure atomicity
        let mut tx = self.pool.begin().await?;

        // Unassign current seat
        sqlx::query(
            r#"
            UPDATE table_seat_assignments
            SET is_current = false,
                unassigned_at = NOW(),
                assigned_by = COALESCE($3, assigned_by),
                updated_at = NOW()
            WHERE tournament_id = $1 AND user_id = $2 AND is_current = true
            "#,
        )
        .bind(tournament_id)
        .bind(user_id)
        .bind(moved_by)
        .execute(&mut *tx)
        .await?;

        // Create new assignment
        let new_assignment = sqlx::query_as::<_, TableSeatAssignmentRow>(
            r#"
            INSERT INTO table_seat_assignments (
                tournament_id, table_id, user_id, seat_number, assigned_by, notes
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, tournament_id, table_id, user_id, seat_number, stack_size, 
                     is_current, assigned_at, unassigned_at, assigned_by, notes, created_at, updated_at
            "#
        )
        .bind(tournament_id)
        .bind(new_table_id)
        .bind(user_id)
        .bind(new_seat_number)
        .bind(moved_by)
        .bind(notes)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(new_assignment)
    }

    /// Count players at a table
    pub async fn count_players_at_table(&self, table_id: Uuid) -> SqlxResult<i64> {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM table_seat_assignments WHERE table_id = $1 AND is_current = true",
        )
        .bind(table_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(result.0)
    }

    /// Get unassigned players (registered but not seated)
    pub async fn get_unassigned_players(&self, tournament_id: Uuid) -> SqlxResult<Vec<UserRow>> {
        sqlx::query_as::<_, UserRow>(
            r#"
            SELECT u.id, u.email, u.username, u.first_name, u.last_name, u.phone, 
                   u.is_active, u.role, u.created_at, u.updated_at
            FROM users u
            JOIN tournament_registrations tr ON u.id = tr.user_id
            LEFT JOIN table_seat_assignments tsa ON u.id = tsa.user_id 
                AND tsa.tournament_id = $1 AND tsa.is_current = true
            WHERE tr.tournament_id = $1 
              AND tr.status = 'registered'
              AND tsa.id IS NULL
            ORDER BY tr.registration_time ASC
            "#,
        )
        .bind(tournament_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Check if a seat is available
    pub async fn is_seat_available(&self, table_id: Uuid, seat_number: i32) -> SqlxResult<bool> {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM table_seat_assignments WHERE table_id = $1 AND seat_number = $2 AND is_current = true"
        )
        .bind(table_id)
        .bind(seat_number)
        .fetch_one(&self.pool)
        .await?;

        Ok(result.0 == 0)
    }
}
