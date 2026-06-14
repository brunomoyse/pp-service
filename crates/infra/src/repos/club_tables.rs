use crate::models::{ClubTableRow, TournamentTableAssignmentRow};
use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct UpdateClubTable {
    pub max_seats: Option<i32>,
    pub is_active: Option<bool>,
    pub is_default: Option<bool>,
}

/// Create a physical table for a club. Fails on the `(club_id, table_number)`
/// unique constraint if the number is already taken.
pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    table_number: i32,
    max_seats: i32,
    is_default: bool,
) -> SqlxResult<ClubTableRow> {
    sqlx::query_as::<_, ClubTableRow>(
        r#"
        INSERT INTO club_tables (club_id, table_number, max_seats, is_default)
        VALUES ($1, $2, $3, $4)
        RETURNING id, club_id, table_number, max_seats, is_active, is_default, created_at, updated_at
        "#,
    )
    .bind(club_id)
    .bind(table_number)
    .bind(max_seats)
    .bind(is_default)
    .fetch_one(executor)
    .await
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<ClubTableRow>> {
    sqlx::query_as::<_, ClubTableRow>(
        r#"
        SELECT id, club_id, table_number, max_seats, is_active, is_default, created_at, updated_at
        FROM club_tables
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn list_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
) -> SqlxResult<Vec<ClubTableRow>> {
    sqlx::query_as::<_, ClubTableRow>(
        r#"
        SELECT id, club_id, table_number, max_seats, is_active, is_default, created_at, updated_at
        FROM club_tables
        WHERE club_id = $1
        ORDER BY table_number ASC
        "#,
    )
    .bind(club_id)
    .fetch_all(executor)
    .await
}

pub async fn list_active_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
) -> SqlxResult<Vec<ClubTableRow>> {
    sqlx::query_as::<_, ClubTableRow>(
        r#"
        SELECT id, club_id, table_number, max_seats, is_active, is_default, created_at, updated_at
        FROM club_tables
        WHERE club_id = $1 AND is_active = true
        ORDER BY table_number ASC
        "#,
    )
    .bind(club_id)
    .fetch_all(executor)
    .await
}

/// Active tables in the club's default set, ordered by number — the set that is
/// auto-linked to a newly created tournament.
pub async fn list_default_active_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
) -> SqlxResult<Vec<ClubTableRow>> {
    sqlx::query_as::<_, ClubTableRow>(
        r#"
        SELECT id, club_id, table_number, max_seats, is_active, is_default, created_at, updated_at
        FROM club_tables
        WHERE club_id = $1 AND is_active = true AND is_default = true
        ORDER BY table_number ASC
        "#,
    )
    .bind(club_id)
    .fetch_all(executor)
    .await
}

pub async fn list_available_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
) -> SqlxResult<Vec<ClubTableRow>> {
    sqlx::query_as::<_, ClubTableRow>(
        r#"
        SELECT ct.id, ct.club_id, ct.table_number, ct.max_seats, ct.is_active, ct.is_default, ct.created_at, ct.updated_at
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
    .fetch_all(executor)
    .await
}

/// A physical table that is already actively booked by another live tournament.
#[derive(Debug, Clone)]
pub struct TableConflict {
    pub club_table_id: Uuid,
    pub table_number: i32,
    pub tournament_id: Uuid,
    pub tournament_name: String,
}

/// Find which of the given tables are actively assigned to a *different*
/// non-finished tournament — i.e. cannot be booked here without double-booking
/// a physical table. `exclude_tournament_id` is the tournament we're assigning
/// to (so its own assignments don't count as conflicts).
pub async fn active_table_conflicts<'e>(
    executor: impl PgExecutor<'e>,
    club_table_ids: &[Uuid],
    exclude_tournament_id: Uuid,
) -> SqlxResult<Vec<TableConflict>> {
    sqlx::query_as::<_, (Uuid, i32, Uuid, String)>(
        r#"
        SELECT ct.id, ct.table_number, t.id, t.name
        FROM tournament_table_assignments tta
        JOIN club_tables ct ON ct.id = tta.club_table_id
        JOIN tournaments t ON t.id = tta.tournament_id
        WHERE tta.club_table_id = ANY($1)
            AND tta.is_active = true
            AND tta.tournament_id <> $2
            AND t.live_status <> 'finished'
        ORDER BY ct.table_number ASC
        "#,
    )
    .bind(club_table_ids)
    .bind(exclude_tournament_id)
    .fetch_all(executor)
    .await
    .map(|rows| {
        rows.into_iter()
            .map(
                |(club_table_id, table_number, tournament_id, tournament_name)| TableConflict {
                    club_table_id,
                    table_number,
                    tournament_id,
                    tournament_name,
                },
            )
            .collect()
    })
}

pub async fn update<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    data: UpdateClubTable,
) -> SqlxResult<Option<ClubTableRow>> {
    sqlx::query_as::<_, ClubTableRow>(
        r#"
        UPDATE club_tables
        SET max_seats = COALESCE($2, max_seats),
            is_active = COALESCE($3, is_active),
            is_default = COALESCE($4, is_default),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, club_id, table_number, max_seats, is_active, is_default, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(data.max_seats)
    .bind(data.is_active)
    .bind(data.is_default)
    .fetch_optional(executor)
    .await
}

pub async fn delete<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> SqlxResult<bool> {
    let result = sqlx::query("DELETE FROM club_tables WHERE id = $1")
        .bind(id)
        .execute(executor)
        .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn assign_to_tournament<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    club_table_id: Uuid,
    max_seats_override: Option<i32>,
) -> SqlxResult<TournamentTableAssignmentRow> {
    sqlx::query_as::<_, TournamentTableAssignmentRow>(
        r#"
        INSERT INTO tournament_table_assignments (tournament_id, club_table_id, max_seats_override)
        VALUES ($1, $2, $3)
        ON CONFLICT (tournament_id, club_table_id) DO UPDATE SET
            is_active = true,
            assigned_at = NOW(),
            deactivated_at = NULL,
            max_seats_override = $3,
            updated_at = NOW()
        RETURNING id, tournament_id, club_table_id, is_active, assigned_at, deactivated_at, max_seats_override, created_at, updated_at
        "#
    )
    .bind(tournament_id)
    .bind(club_table_id)
    .bind(max_seats_override)
    .fetch_one(executor)
    .await
}

pub async fn unassign_from_tournament<'e>(
    executor: impl PgExecutor<'e>,
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
    .execute(executor)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn list_assigned_to_tournament<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> SqlxResult<Vec<ClubTableRow>> {
    sqlx::query_as::<_, ClubTableRow>(
        r#"
        SELECT ct.id, ct.club_id, ct.table_number,
               COALESCE(tta.max_seats_override, ct.max_seats) as max_seats,
               ct.is_active, ct.is_default, ct.created_at, ct.updated_at
        FROM club_tables ct
        INNER JOIN tournament_table_assignments tta ON ct.id = tta.club_table_id
        WHERE tta.tournament_id = $1 AND tta.is_active = true
        ORDER BY ct.table_number ASC
        "#,
    )
    .bind(tournament_id)
    .fetch_all(executor)
    .await
}
