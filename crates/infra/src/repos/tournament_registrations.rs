use sqlx::{PgExecutor, Result};
use uuid::Uuid;

use crate::models::TournamentRegistrationRow;

#[derive(Debug, Clone)]
pub struct CreateTournamentRegistration {
    pub tournament_id: Uuid,
    pub user_id: Uuid,
    pub notes: Option<String>,
    pub status: Option<String>,
}

pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    data: CreateTournamentRegistration,
) -> Result<TournamentRegistrationRow> {
    let row = sqlx::query_as::<_, TournamentRegistrationRow>(
        r#"
        INSERT INTO tournament_registrations (tournament_id, user_id, notes, status)
        VALUES ($1, $2, $3, COALESCE($4, 'registered'))
        RETURNING id, tournament_id, user_id, registration_time, status, notes, created_at, updated_at
        "#
    )
    .bind(data.tournament_id)
    .bind(data.user_id)
    .bind(data.notes)
    .bind(data.status)
    .fetch_one(executor)
    .await?;

    Ok(row)
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> Result<Option<TournamentRegistrationRow>> {
    let row = sqlx::query_as::<_, TournamentRegistrationRow>(
        r#"
        SELECT id, tournament_id, user_id, registration_time, status, notes, created_at, updated_at
        FROM tournament_registrations
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

pub async fn get_by_tournament_and_user<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    user_id: Uuid,
) -> Result<Option<TournamentRegistrationRow>> {
    let row = sqlx::query_as::<_, TournamentRegistrationRow>(
        r#"
        SELECT id, tournament_id, user_id, registration_time, status, notes, created_at, updated_at
        FROM tournament_registrations
        WHERE tournament_id = $1 AND user_id = $2
        "#,
    )
    .bind(tournament_id)
    .bind(user_id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

pub async fn list_by_tournament<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> Result<Vec<TournamentRegistrationRow>> {
    let rows = sqlx::query_as::<_, TournamentRegistrationRow>(
        r#"
        SELECT id, tournament_id, user_id, registration_time, status, notes, created_at, updated_at
        FROM tournament_registrations
        WHERE tournament_id = $1
        ORDER BY registration_time ASC
        "#,
    )
    .bind(tournament_id)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

pub async fn list_by_tournament_paginated<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    page: crate::pagination::LimitOffset,
) -> Result<Vec<TournamentRegistrationRow>> {
    let rows = sqlx::query_as::<_, TournamentRegistrationRow>(
        r#"
        SELECT id, tournament_id, user_id, registration_time, status, notes, created_at, updated_at
        FROM tournament_registrations
        WHERE tournament_id = $1
        ORDER BY registration_time ASC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(tournament_id)
    .bind(page.limit)
    .bind(page.offset)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

pub async fn count_by_tournament<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> Result<i64> {
    let result = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM tournament_registrations
        WHERE tournament_id = $1
        "#,
    )
    .bind(tournament_id)
    .fetch_one(executor)
    .await?;

    Ok(result)
}

pub async fn list_user_current<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
) -> Result<Vec<TournamentRegistrationRow>> {
    let rows = sqlx::query_as::<_, TournamentRegistrationRow>(
        r#"
        SELECT tr.id, tr.tournament_id, tr.user_id, tr.registration_time, tr.status, tr.notes, tr.created_at, tr.updated_at
        FROM tournament_registrations tr
        JOIN tournaments t ON tr.tournament_id = t.id
        WHERE tr.user_id = $1 AND (t.end_time IS NULL OR t.end_time > NOW())
        ORDER BY tr.created_at DESC
        "#
    )
    .bind(user_id)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

pub async fn update_status<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    user_id: Uuid,
    status: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE tournament_registrations SET status = $3, updated_at = NOW() WHERE tournament_id = $1 AND user_id = $2",
    )
    .bind(tournament_id)
    .bind(user_id)
    .bind(status)
    .execute(executor)
    .await?;
    Ok(())
}

/// Count confirmed registrations (those occupying a seat: registered, checked_in, seated, busted).
/// Waitlisted, cancelled, and no_show do not count.
pub async fn count_confirmed_by_tournament<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> Result<i64> {
    let result = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM tournament_registrations
        WHERE tournament_id = $1 AND status IN ('registered', 'checked_in', 'seated', 'busted')
        "#,
    )
    .bind(tournament_id)
    .fetch_one(executor)
    .await?;

    Ok(result)
}

/// Get the next waitlisted player (FIFO by registration_time).
pub async fn get_next_waitlisted<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> Result<Option<TournamentRegistrationRow>> {
    let row = sqlx::query_as::<_, TournamentRegistrationRow>(
        r#"
        SELECT id, tournament_id, user_id, registration_time, status, notes, created_at, updated_at
        FROM tournament_registrations
        WHERE tournament_id = $1 AND status = 'waitlisted'
        ORDER BY registration_time ASC
        LIMIT 1
        "#,
    )
    .bind(tournament_id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

/// Get a player's position in the waitlist (1-based). Returns None if not waitlisted.
pub async fn get_waitlist_position<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    user_id: Uuid,
) -> Result<Option<i64>> {
    let result = sqlx::query_scalar::<_, Option<i64>>(
        r#"
        SELECT position FROM (
            SELECT user_id, ROW_NUMBER() OVER (ORDER BY registration_time ASC) as position
            FROM tournament_registrations
            WHERE tournament_id = $1 AND status = 'waitlisted'
        ) ranked
        WHERE user_id = $2
        "#,
    )
    .bind(tournament_id)
    .bind(user_id)
    .fetch_optional(executor)
    .await?;

    Ok(result.flatten())
}
