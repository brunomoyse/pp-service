use sqlx::{PgExecutor, PgPool, Result};
use uuid::Uuid;

use crate::models::TournamentPayoutRow;

pub async fn get_by_tournament<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> Result<Option<TournamentPayoutRow>> {
    let row = sqlx::query_as::<_, TournamentPayoutRow>(
        r#"
        SELECT id, tournament_id, template_id, player_count,
               total_prize_pool, payout_positions, created_at, updated_at
        FROM tournament_payouts
        WHERE tournament_id = $1
        "#,
    )
    .bind(tournament_id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> Result<Option<TournamentPayoutRow>> {
    let row = sqlx::query_as::<_, TournamentPayoutRow>(
        r#"
        SELECT id, tournament_id, template_id, player_count,
               total_prize_pool, payout_positions, created_at, updated_at
        FROM tournament_payouts
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

pub async fn recalculate(
    pool: &PgPool,
    tournament_id: Uuid,
) -> Result<Option<TournamentPayoutRow>> {
    // Delete existing payout
    sqlx::query("DELETE FROM tournament_payouts WHERE tournament_id = $1")
        .bind(tournament_id)
        .execute(pool)
        .await?;

    // Trigger recalculation by updating tournament status
    sqlx::query(
        r#"
        UPDATE tournaments
        SET live_status = 'in_progress', updated_at = NOW()
        WHERE id = $1 AND live_status = 'in_progress'
        "#,
    )
    .bind(tournament_id)
    .execute(pool)
    .await?;

    // Return the new payout
    get_by_tournament(pool, tournament_id).await
}

pub async fn update_positions<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    payout_positions: serde_json::Value,
) -> Result<TournamentPayoutRow> {
    let row = sqlx::query_as::<_, TournamentPayoutRow>(
        r#"
        UPDATE tournament_payouts
        SET payout_positions = $2, updated_at = NOW()
        WHERE tournament_id = $1
        RETURNING id, tournament_id, template_id, player_count,
                  total_prize_pool, payout_positions, created_at, updated_at
        "#,
    )
    .bind(tournament_id)
    .bind(payout_positions)
    .fetch_one(executor)
    .await?;

    Ok(row)
}

pub async fn delete<'e>(executor: impl PgExecutor<'e>, tournament_id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM tournament_payouts WHERE tournament_id = $1")
        .bind(tournament_id)
        .execute(executor)
        .await?;

    Ok(result.rows_affected() > 0)
}
