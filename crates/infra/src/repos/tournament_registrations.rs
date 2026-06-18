use sqlx::{PgExecutor, Result};
use uuid::Uuid;

use crate::models::TournamentRegistrationRow;

const COLS: &str =
    "id, tournament_id, user_id, club_player_id, registration_time, status, notes, current_bounty_cents, starting_stack, created_at, updated_at";

#[derive(Debug, Clone, Default)]
pub struct CreateTournamentRegistration {
    pub tournament_id: Uuid,
    /// App user, when the player has an account. Optional — account-less players
    /// (registered by the club) carry only a roster id. The link trigger stamps
    /// whichever of user_id / club_player_id is missing.
    pub user_id: Option<Uuid>,
    pub club_player_id: Option<Uuid>,
    pub notes: Option<String>,
    pub status: Option<String>,
}

pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    data: CreateTournamentRegistration,
) -> Result<TournamentRegistrationRow> {
    let row = sqlx::query_as::<_, TournamentRegistrationRow>(
        r#"
        INSERT INTO tournament_registrations (tournament_id, user_id, club_player_id, notes, status)
        VALUES ($1, $2, $3, $4, COALESCE($5, 'registered'))
        RETURNING id, tournament_id, user_id, club_player_id, registration_time, status, notes, current_bounty_cents, starting_stack, created_at, updated_at
        "#
    )
    .bind(data.tournament_id)
    .bind(data.user_id)
    .bind(data.club_player_id)
    .bind(data.notes)
    .bind(data.status)
    .fetch_one(executor)
    .await?;

    Ok(row)
}

/// Seed a final-day registration for a Day-2 qualifier: status `checked_in`
/// with the carried-over `starting_stack`. Idempotent on
/// (tournament_id, club_player_id): re-running updates the stack (best stack
/// forward) without creating a duplicate registration.
pub async fn upsert_checked_in_with_stack<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    club_player_id: Uuid,
    starting_stack: i32,
) -> Result<TournamentRegistrationRow> {
    let row = sqlx::query_as::<_, TournamentRegistrationRow>(
        r#"
        INSERT INTO tournament_registrations (tournament_id, club_player_id, status, starting_stack)
        VALUES ($1, $2, 'checked_in', $3)
        ON CONFLICT (tournament_id, club_player_id) DO UPDATE SET
            status = 'checked_in',
            starting_stack = EXCLUDED.starting_stack,
            updated_at = NOW()
        RETURNING id, tournament_id, user_id, club_player_id, registration_time, status, notes, current_bounty_cents, starting_stack, created_at, updated_at
        "#,
    )
    .bind(tournament_id)
    .bind(club_player_id)
    .bind(starting_stack)
    .fetch_one(executor)
    .await?;

    Ok(row)
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> Result<Option<TournamentRegistrationRow>> {
    let row = sqlx::query_as::<_, TournamentRegistrationRow>(&format!(
        "SELECT {COLS} FROM tournament_registrations WHERE id = $1"
    ))
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
    let row = sqlx::query_as::<_, TournamentRegistrationRow>(&format!(
        "SELECT {COLS} FROM tournament_registrations WHERE tournament_id = $1 AND user_id = $2"
    ))
    .bind(tournament_id)
    .bind(user_id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

pub async fn get_by_tournament_and_club_player<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    club_player_id: Uuid,
) -> Result<Option<TournamentRegistrationRow>> {
    let row = sqlx::query_as::<_, TournamentRegistrationRow>(&format!(
        "SELECT {COLS} FROM tournament_registrations WHERE tournament_id = $1 AND club_player_id = $2"
    ))
    .bind(tournament_id)
    .bind(club_player_id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

pub async fn list_by_tournament<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> Result<Vec<TournamentRegistrationRow>> {
    let rows = sqlx::query_as::<_, TournamentRegistrationRow>(&format!(
        "SELECT {COLS} FROM tournament_registrations WHERE tournament_id = $1 ORDER BY registration_time ASC"
    ))
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
    let rows = sqlx::query_as::<_, TournamentRegistrationRow>(&format!(
        "SELECT {COLS} FROM tournament_registrations WHERE tournament_id = $1 ORDER BY registration_time ASC LIMIT $2 OFFSET $3"
    ))
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
        "SELECT COUNT(*) FROM tournament_registrations WHERE tournament_id = $1",
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
        "SELECT tr.id, tr.tournament_id, tr.user_id, tr.club_player_id, tr.registration_time, \
                tr.status, tr.notes, tr.current_bounty_cents, tr.starting_stack, tr.created_at, tr.updated_at \
         FROM tournament_registrations tr \
         JOIN tournaments t ON tr.tournament_id = t.id \
         WHERE tr.user_id = $1 AND (t.end_time IS NULL OR t.end_time > NOW()) \
           AND t.club_id NOT IN (SELECT id FROM clubs WHERE plan = 'free') \
         ORDER BY tr.created_at DESC",
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

/// Update status keyed on the roster identity (works for account-less players).
pub async fn update_status_by_club_player<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    club_player_id: Uuid,
    status: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE tournament_registrations SET status = $3, updated_at = NOW() WHERE tournament_id = $1 AND club_player_id = $2",
    )
    .bind(tournament_id)
    .bind(club_player_id)
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
    let row = sqlx::query_as::<_, TournamentRegistrationRow>(&format!(
        "SELECT {COLS} FROM tournament_registrations WHERE tournament_id = $1 AND status = 'waitlisted' ORDER BY registration_time ASC LIMIT 1"
    ))
    .bind(tournament_id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

/// Get a player's position in the waitlist (1-based). Returns None if not waitlisted.
pub async fn get_waitlist_position<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    club_player_id: Uuid,
) -> Result<Option<i64>> {
    let result = sqlx::query_scalar::<_, Option<i64>>(
        r#"
        SELECT position FROM (
            SELECT club_player_id, ROW_NUMBER() OVER (ORDER BY registration_time ASC) as position
            FROM tournament_registrations
            WHERE tournament_id = $1 AND status = 'waitlisted'
        ) ranked
        WHERE club_player_id = $2
        "#,
    )
    .bind(tournament_id)
    .bind(club_player_id)
    .fetch_optional(executor)
    .await?;

    Ok(result.flatten())
}
