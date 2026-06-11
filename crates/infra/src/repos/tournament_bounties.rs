//! Bounty / progressive-knockout (PKO) bookkeeping.
//!
//! A knockout pays the hunter a cash bounty and, for progressive tournaments,
//! grows the hunter's own head. The per-knockout audit lives in
//! `tournament_bounties`; the live head per player lives on
//! `tournament_registrations.current_bounty_cents`.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct BountyRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub hunter_club_player_id: Uuid,
    pub victim_club_player_id: Uuid,
    pub amount_cents: i32,
    pub created_at: DateTime<Utc>,
}

/// Result of recording a knockout.
#[derive(Debug, Clone, Copy)]
pub struct KnockoutOutcome {
    /// Cash bounty the hunter collected, in cents.
    pub cash_cents: i32,
    /// Amount added to the hunter's progressive head (0 for fixed bounties).
    pub head_growth_cents: i32,
}

/// Record a knockout bounty inside the elimination transaction.
///
/// `bounty_type` / `bounty_amount_cents` come from the tournament. Money model:
/// - **fixed**: the hunter collects the full `bounty_amount_cents`; no head growth.
/// - **progressive**: the hunter collects half of the victim's current head
///   (floored); the remainder grows the hunter's own head, and the victim's head
///   is zeroed.
///
/// Returns `None` (and records nothing) when the tournament has no active bounty
/// or the victim has no head to collect.
pub async fn record_knockout(
    conn: &mut sqlx::PgConnection,
    tournament_id: Uuid,
    hunter_club_player_id: Uuid,
    victim_club_player_id: Uuid,
    bounty_type: &str,
    bounty_amount_cents: i32,
) -> Result<Option<KnockoutOutcome>, sqlx::Error> {
    if bounty_amount_cents <= 0 || bounty_type == "none" {
        return Ok(None);
    }

    // The victim's head: their accumulated progressive head, or the fixed slice.
    let head: i32 = if bounty_type == "progressive" {
        sqlx::query_scalar(
            "SELECT current_bounty_cents FROM tournament_registrations \
             WHERE tournament_id = $1 AND club_player_id = $2",
        )
        .bind(tournament_id)
        .bind(victim_club_player_id)
        .fetch_optional(&mut *conn)
        .await?
        .unwrap_or(0)
    } else {
        bounty_amount_cents
    };

    if head <= 0 {
        return Ok(None);
    }

    let (cash_cents, head_growth_cents) = if bounty_type == "progressive" {
        let cash = head / 2;
        (cash, head - cash)
    } else {
        (head, 0)
    };

    sqlx::query(
        "INSERT INTO tournament_bounties \
         (tournament_id, hunter_club_player_id, victim_club_player_id, amount_cents) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(tournament_id)
    .bind(hunter_club_player_id)
    .bind(victim_club_player_id)
    .bind(cash_cents)
    .execute(&mut *conn)
    .await?;

    if bounty_type == "progressive" {
        // Grow the hunter's head, then zero the busted victim's.
        sqlx::query(
            "UPDATE tournament_registrations \
             SET current_bounty_cents = current_bounty_cents + $3, updated_at = NOW() \
             WHERE tournament_id = $1 AND club_player_id = $2",
        )
        .bind(tournament_id)
        .bind(hunter_club_player_id)
        .bind(head_growth_cents)
        .execute(&mut *conn)
        .await?;

        sqlx::query(
            "UPDATE tournament_registrations \
             SET current_bounty_cents = 0, updated_at = NOW() \
             WHERE tournament_id = $1 AND club_player_id = $2",
        )
        .bind(tournament_id)
        .bind(victim_club_player_id)
        .execute(&mut *conn)
        .await?;
    }

    Ok(Some(KnockoutOutcome {
        cash_cents,
        head_growth_cents,
    }))
}

/// All knockouts recorded for a tournament, most recent first.
pub async fn list_by_tournament<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    tournament_id: Uuid,
) -> Result<Vec<BountyRow>, sqlx::Error> {
    sqlx::query_as::<_, BountyRow>(
        "SELECT id, tournament_id, hunter_club_player_id, victim_club_player_id, \
                amount_cents, created_at \
         FROM tournament_bounties WHERE tournament_id = $1 \
         ORDER BY created_at DESC",
    )
    .bind(tournament_id)
    .fetch_all(executor)
    .await
}
