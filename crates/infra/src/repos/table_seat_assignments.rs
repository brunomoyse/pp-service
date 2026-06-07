use crate::models::{TableSeatAssignmentRow, UserRow};
use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, PgPool, Result as SqlxResult};
use uuid::Uuid;

const COLS: &str = "id, tournament_id, club_table_id, user_id, registered_player_id, seat_number, stack_size, is_current, assigned_at, unassigned_at, assigned_by, notes, created_at, updated_at";

#[derive(Debug, Clone, Default)]
pub struct CreateSeatAssignment {
    pub tournament_id: Uuid,
    pub club_table_id: Uuid,
    /// App user, when the player has an account. The link trigger stamps
    /// whichever of user_id / registered_player_id is missing.
    pub user_id: Option<Uuid>,
    pub registered_player_id: Option<Uuid>,
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
    /// Always rendered (roster display name).
    pub display_name: String,
    /// The app user, when the seated player has an account.
    pub player: Option<UserRow>,
}

#[derive(Debug, Clone)]
pub struct SeatAssignmentFilter {
    pub tournament_id: Option<Uuid>,
    pub club_table_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub is_current: Option<bool>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
}

pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    data: CreateSeatAssignment,
) -> SqlxResult<TableSeatAssignmentRow> {
    sqlx::query_as::<_, TableSeatAssignmentRow>(&format!(
        "INSERT INTO table_seat_assignments \
            (tournament_id, club_table_id, user_id, registered_player_id, seat_number, stack_size, assigned_by, notes) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING {COLS}"
    ))
    .bind(data.tournament_id)
    .bind(data.club_table_id)
    .bind(data.user_id)
    .bind(data.registered_player_id)
    .bind(data.seat_number)
    .bind(data.stack_size)
    .bind(data.assigned_by)
    .bind(data.notes)
    .fetch_one(executor)
    .await
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<TableSeatAssignmentRow>> {
    sqlx::query_as::<_, TableSeatAssignmentRow>(&format!(
        "SELECT {COLS} FROM table_seat_assignments WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn get_current_for_user<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    user_id: Uuid,
) -> SqlxResult<Option<TableSeatAssignmentRow>> {
    sqlx::query_as::<_, TableSeatAssignmentRow>(&format!(
        "SELECT {COLS} FROM table_seat_assignments WHERE tournament_id = $1 AND user_id = $2 AND is_current = true"
    ))
    .bind(tournament_id)
    .bind(user_id)
    .fetch_optional(executor)
    .await
}

pub async fn list_current_for_table<'e>(
    executor: impl PgExecutor<'e>,
    club_table_id: Uuid,
) -> SqlxResult<Vec<TableSeatAssignmentRow>> {
    sqlx::query_as::<_, TableSeatAssignmentRow>(&format!(
        "SELECT {COLS} FROM table_seat_assignments WHERE club_table_id = $1 AND is_current = true ORDER BY seat_number ASC"
    ))
    .bind(club_table_id)
    .fetch_all(executor)
    .await
}

pub async fn list_current_for_tournament<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> SqlxResult<Vec<TableSeatAssignmentRow>> {
    sqlx::query_as::<_, TableSeatAssignmentRow>(&format!(
        "SELECT {COLS} FROM table_seat_assignments WHERE tournament_id = $1 AND is_current = true ORDER BY club_table_id, seat_number ASC"
    ))
    .bind(tournament_id)
    .fetch_all(executor)
    .await
}

pub async fn list_current_for_tournament_table<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    club_table_id: Uuid,
) -> SqlxResult<Vec<TableSeatAssignmentRow>> {
    sqlx::query_as::<_, TableSeatAssignmentRow>(&format!(
        "SELECT {COLS} FROM table_seat_assignments WHERE tournament_id = $1 AND club_table_id = $2 AND is_current = true ORDER BY seat_number ASC"
    ))
    .bind(tournament_id)
    .bind(club_table_id)
    .fetch_all(executor)
    .await
}

pub async fn list_current_with_players_for_table<'e>(
    executor: impl PgExecutor<'e>,
    club_table_id: Uuid,
) -> SqlxResult<Vec<SeatAssignmentWithPlayer>> {
    #[derive(sqlx::FromRow)]
    struct JoinedRow {
        // Assignment fields
        id: Uuid,
        tournament_id: Uuid,
        club_table_id: Uuid,
        user_id: Option<Uuid>,
        registered_player_id: Uuid,
        seat_number: i32,
        stack_size: Option<i32>,
        is_current: bool,
        assigned_at: DateTime<Utc>,
        unassigned_at: Option<DateTime<Utc>>,
        assigned_by: Option<Uuid>,
        notes: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        // Roster + (optional) user fields
        display_name: String,
        email: Option<String>,
        username: Option<String>,
        first_name: Option<String>,
        last_name: Option<String>,
        phone: Option<String>,
        is_active: Option<bool>,
        role: Option<String>,
        locale: Option<String>,
        user_created_at: Option<DateTime<Utc>>,
        user_updated_at: Option<DateTime<Utc>>,
    }

    let rows = sqlx::query_as::<_, JoinedRow>(
        r#"
        SELECT
            tsa.id, tsa.tournament_id, tsa.club_table_id, tsa.user_id, tsa.registered_player_id,
            tsa.seat_number, tsa.stack_size, tsa.is_current, tsa.assigned_at, tsa.unassigned_at,
            tsa.assigned_by, tsa.notes, tsa.created_at, tsa.updated_at,
            rp.display_name,
            u.email, u.username, u.first_name, u.last_name, u.phone, u.is_active, u.role,
            u.locale, u.created_at as user_created_at, u.updated_at as user_updated_at
        FROM table_seat_assignments tsa
        JOIN registered_player rp ON tsa.registered_player_id = rp.id
        LEFT JOIN users u ON tsa.user_id = u.id
        WHERE tsa.club_table_id = $1 AND tsa.is_current = true
        ORDER BY tsa.seat_number ASC
        "#,
    )
    .bind(club_table_id)
    .fetch_all(executor)
    .await?;

    let results = rows
        .into_iter()
        .map(|row| {
            let player = match (row.user_id, row.first_name.clone(), row.email.clone()) {
                (Some(uid), Some(first_name), Some(email)) => Some(UserRow {
                    id: uid,
                    email,
                    username: row.username.clone(),
                    first_name,
                    last_name: row.last_name.clone(),
                    phone: row.phone.clone(),
                    is_active: row.is_active.unwrap_or(true),
                    role: row.role.clone(),
                    locale: row.locale.clone().unwrap_or_else(|| "en".to_string()),
                    created_at: row.user_created_at.unwrap_or(row.created_at),
                    updated_at: row.user_updated_at.unwrap_or(row.updated_at),
                }),
                _ => None,
            };
            SeatAssignmentWithPlayer {
                assignment: TableSeatAssignmentRow {
                    id: row.id,
                    tournament_id: row.tournament_id,
                    club_table_id: row.club_table_id,
                    user_id: row.user_id,
                    registered_player_id: row.registered_player_id,
                    seat_number: row.seat_number,
                    stack_size: row.stack_size,
                    is_current: row.is_current,
                    assigned_at: row.assigned_at,
                    unassigned_at: row.unassigned_at,
                    assigned_by: row.assigned_by,
                    notes: row.notes,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                },
                display_name: row.display_name,
                player,
            }
        })
        .collect();

    Ok(results)
}

pub async fn list_history<'e>(
    executor: impl PgExecutor<'e>,
    filter: SeatAssignmentFilter,
    limit: Option<i64>,
) -> SqlxResult<Vec<TableSeatAssignmentRow>> {
    let limit = limit.unwrap_or(100).min(1000);

    sqlx::query_as::<_, TableSeatAssignmentRow>(&format!(
        "SELECT {COLS} FROM table_seat_assignments \
         WHERE ($1::uuid IS NULL OR tournament_id = $1) \
           AND ($2::uuid IS NULL OR club_table_id = $2) \
           AND ($3::uuid IS NULL OR user_id = $3) \
           AND ($4::boolean IS NULL OR is_current = $4) \
           AND ($5::timestamptz IS NULL OR assigned_at >= $5) \
           AND ($6::timestamptz IS NULL OR assigned_at <= $6) \
         ORDER BY assigned_at DESC LIMIT $7"
    ))
    .bind(filter.tournament_id)
    .bind(filter.club_table_id)
    .bind(filter.user_id)
    .bind(filter.is_current)
    .bind(filter.from_date)
    .bind(filter.to_date)
    .bind(limit)
    .fetch_all(executor)
    .await
}

pub async fn update<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    data: UpdateSeatAssignment,
) -> SqlxResult<Option<TableSeatAssignmentRow>> {
    sqlx::query_as::<_, TableSeatAssignmentRow>(&format!(
        "UPDATE table_seat_assignments \
         SET stack_size = COALESCE($2, stack_size), notes = COALESCE($3, notes), updated_at = NOW() \
         WHERE id = $1 RETURNING {COLS}"
    ))
    .bind(id)
    .bind(data.stack_size)
    .bind(data.notes)
    .fetch_optional(executor)
    .await
}

pub async fn unassign<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    unassigned_by: Option<Uuid>,
) -> SqlxResult<Option<TableSeatAssignmentRow>> {
    sqlx::query_as::<_, TableSeatAssignmentRow>(&format!(
        "UPDATE table_seat_assignments \
         SET is_current = false, unassigned_at = NOW(), assigned_by = COALESCE($2, assigned_by), updated_at = NOW() \
         WHERE id = $1 RETURNING {COLS}"
    ))
    .bind(id)
    .bind(unassigned_by)
    .fetch_optional(executor)
    .await
}

pub async fn unassign_current_seat<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    registered_player_id: Uuid,
    moved_by: Option<Uuid>,
) -> SqlxResult<()> {
    sqlx::query(
        r#"
        UPDATE table_seat_assignments
        SET is_current = false,
            unassigned_at = NOW(),
            assigned_by = COALESCE($3, assigned_by),
            updated_at = NOW()
        WHERE tournament_id = $1 AND registered_player_id = $2 AND is_current = true
        "#,
    )
    .bind(tournament_id)
    .bind(registered_player_id)
    .bind(moved_by)
    .execute(executor)
    .await?;
    Ok(())
}

/// Move a player to a new seat (creates new assignment and unassigns old one).
/// Uses a transaction so requires &PgPool. Keyed on the roster identity.
pub async fn move_player(
    pool: &PgPool,
    tournament_id: Uuid,
    registered_player_id: Uuid,
    new_club_table_id: Uuid,
    new_seat_number: i32,
    moved_by: Option<Uuid>,
    notes: Option<String>,
) -> SqlxResult<TableSeatAssignmentRow> {
    let mut tx = pool.begin().await?;
    unassign_current_seat(&mut *tx, tournament_id, registered_player_id, moved_by).await?;
    let result = create_seat_in_tx(
        &mut *tx,
        tournament_id,
        registered_player_id,
        new_club_table_id,
        new_seat_number,
        moved_by,
        notes,
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}

/// Insert a new seat assignment (used within transactions)
async fn create_seat_in_tx<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    registered_player_id: Uuid,
    new_club_table_id: Uuid,
    new_seat_number: i32,
    moved_by: Option<Uuid>,
    notes: Option<String>,
) -> SqlxResult<TableSeatAssignmentRow> {
    sqlx::query_as::<_, TableSeatAssignmentRow>(&format!(
        "INSERT INTO table_seat_assignments \
            (tournament_id, club_table_id, registered_player_id, seat_number, assigned_by, notes) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING {COLS}"
    ))
    .bind(tournament_id)
    .bind(new_club_table_id)
    .bind(registered_player_id)
    .bind(new_seat_number)
    .bind(moved_by)
    .bind(notes)
    .fetch_one(executor)
    .await
}

pub async fn count_players_at_table<'e>(
    executor: impl PgExecutor<'e>,
    club_table_id: Uuid,
) -> SqlxResult<i64> {
    let result: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM table_seat_assignments WHERE club_table_id = $1 AND is_current = true",
    )
    .bind(club_table_id)
    .fetch_one(executor)
    .await?;

    Ok(result.0)
}

/// Roster players registered (and not busted/cancelled) for the tournament who
/// do not currently hold a seat.
pub async fn list_unassigned_players<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> SqlxResult<Vec<crate::models::RegisteredPlayerRow>> {
    sqlx::query_as::<_, crate::models::RegisteredPlayerRow>(
        r#"
        SELECT rp.id, rp.club_id, rp.display_name, rp.app_user_id, rp.created_at, rp.updated_at
        FROM tournament_registrations tr
        JOIN registered_player rp ON rp.id = tr.registered_player_id
        LEFT JOIN table_seat_assignments tsa ON tsa.registered_player_id = rp.id
            AND tsa.tournament_id = $1 AND tsa.is_current = true
        WHERE tr.tournament_id = $1
          AND tr.status IN ('registered', 'checked_in', 'seated')
          AND tsa.id IS NULL
        ORDER BY tr.registration_time ASC
        "#,
    )
    .bind(tournament_id)
    .fetch_all(executor)
    .await
}

pub async fn is_seat_available<'e>(
    executor: impl PgExecutor<'e>,
    club_table_id: Uuid,
    seat_number: i32,
) -> SqlxResult<bool> {
    let result: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM table_seat_assignments WHERE club_table_id = $1 AND seat_number = $2 AND is_current = true"
    )
    .bind(club_table_id)
    .bind(seat_number)
    .fetch_one(executor)
    .await?;

    Ok(result.0 == 0)
}

pub async fn get_occupied_seats<'e>(
    executor: impl PgExecutor<'e>,
    club_table_id: Uuid,
) -> SqlxResult<Vec<i32>> {
    let rows: Vec<(i32,)> = sqlx::query_as(
        r#"
        SELECT seat_number
        FROM table_seat_assignments
        WHERE club_table_id = $1 AND is_current = true
        ORDER BY seat_number
        "#,
    )
    .bind(club_table_id)
    .fetch_all(executor)
    .await?;

    Ok(rows.into_iter().map(|(seat,)| seat).collect())
}
