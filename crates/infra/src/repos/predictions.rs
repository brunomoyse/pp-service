use sqlx::{PgExecutor, PgPool, Result as SqlxResult};
use uuid::Uuid;

use crate::models::{PredictionEntryRow, PredictionEntryView};

// ---- Prediction-Points ledger (earned-only) ----

/// Current PP balance = sum of all ledger deltas.
pub async fn balance<'e>(executor: impl PgExecutor<'e>, app_user_id: Uuid) -> SqlxResult<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(delta), 0) FROM prediction_point_ledger WHERE app_user_id = $1",
    )
    .bind(app_user_id)
    .fetch_one(executor)
    .await?;
    Ok(row.0)
}

/// Total PP already credited under a given reason (e.g. 'earned').
pub async fn credited_for_reason<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    reason: &str,
) -> SqlxResult<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(delta), 0) FROM prediction_point_ledger \
         WHERE app_user_id = $1 AND reason = $2",
    )
    .bind(app_user_id)
    .bind(reason)
    .fetch_one(executor)
    .await?;
    Ok(row.0)
}

pub async fn has_reason<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    reason: &str,
) -> SqlxResult<bool> {
    let row: (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM prediction_point_ledger WHERE app_user_id = $1 AND reason = $2)",
    )
    .bind(app_user_id)
    .bind(reason)
    .fetch_one(executor)
    .await?;
    Ok(row.0)
}

pub async fn insert_ledger<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    delta: i32,
    reason: &str,
    ref_id: Option<Uuid>,
) -> SqlxResult<()> {
    sqlx::query(
        "INSERT INTO prediction_point_ledger (app_user_id, delta, reason, ref_id) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(app_user_id)
    .bind(delta)
    .bind(reason)
    .bind(ref_id)
    .execute(executor)
    .await?;
    Ok(())
}

/// Lifetime check-ins (drives earned PP — ties PP to attendance, never euros).
pub async fn check_in_total<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
) -> SqlxResult<i64> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM check_in WHERE app_user_id = $1")
        .bind(app_user_id)
        .fetch_one(executor)
        .await?;
    Ok(row.0)
}

/// Lifetime tournaments played (drives earned PP).
pub async fn tournaments_total<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
) -> SqlxResult<i64> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tournament_results WHERE user_id = $1")
        .bind(app_user_id)
        .fetch_one(executor)
        .await?;
    Ok(row.0)
}

// ---- Prediction entries (fantasy picks) ----

pub async fn get_entry<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    tournament_id: Uuid,
) -> SqlxResult<Option<PredictionEntryRow>> {
    sqlx::query_as::<_, PredictionEntryRow>(
        "SELECT id, app_user_id, tournament_id, predicted_winner_user_id, stake_points, \
                status, payout_points, created_at, resolved_at \
         FROM prediction_entry WHERE app_user_id = $1 AND tournament_id = $2",
    )
    .bind(app_user_id)
    .bind(tournament_id)
    .fetch_optional(executor)
    .await
}

/// Place a fantasy pick: stakes PP and records the entry atomically.
pub async fn create_entry(
    pool: &PgPool,
    app_user_id: Uuid,
    tournament_id: Uuid,
    predicted_winner_user_id: Uuid,
    stake_points: i32,
) -> SqlxResult<PredictionEntryRow> {
    let mut tx = pool.begin().await?;

    let entry = sqlx::query_as::<_, PredictionEntryRow>(
        "INSERT INTO prediction_entry \
            (app_user_id, tournament_id, predicted_winner_user_id, stake_points) \
         VALUES ($1, $2, $3, $4) \
         RETURNING id, app_user_id, tournament_id, predicted_winner_user_id, stake_points, \
                   status, payout_points, created_at, resolved_at",
    )
    .bind(app_user_id)
    .bind(tournament_id)
    .bind(predicted_winner_user_id)
    .bind(stake_points)
    .fetch_one(&mut *tx)
    .await?;

    insert_ledger(
        &mut *tx,
        app_user_id,
        -stake_points,
        "prediction_stake",
        Some(entry.id),
    )
    .await?;

    tx.commit().await?;
    Ok(entry)
}

/// The current user's predictions, enriched with names, newest first.
pub async fn list_for_user<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
) -> SqlxResult<Vec<PredictionEntryView>> {
    sqlx::query_as::<_, PredictionEntryView>(
        "SELECT pe.id AS id, pe.tournament_id AS tournament_id, \
                t.name AS tournament_name, \
                COALESCE(w.username, w.first_name) AS predicted_winner_name, \
                pe.stake_points AS stake_points, pe.status AS status, \
                pe.payout_points AS payout_points, pe.created_at AS created_at \
         FROM prediction_entry pe \
         JOIN tournaments t ON t.id = pe.tournament_id \
         JOIN users w ON w.id = pe.predicted_winner_user_id \
         WHERE pe.app_user_id = $1 \
         ORDER BY pe.created_at DESC",
    )
    .bind(app_user_id)
    .fetch_all(executor)
    .await
}

/// Open predictions for a tournament (used at resolution).
pub async fn open_for_tournament<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> SqlxResult<Vec<PredictionEntryRow>> {
    sqlx::query_as::<_, PredictionEntryRow>(
        "SELECT id, app_user_id, tournament_id, predicted_winner_user_id, stake_points, \
                status, payout_points, created_at, resolved_at \
         FROM prediction_entry WHERE tournament_id = $1 AND status = 'open'",
    )
    .bind(tournament_id)
    .fetch_all(executor)
    .await
}

/// The tournament winner's user id (final position 1), if results are entered.
pub async fn winner_user_id<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> SqlxResult<Option<Uuid>> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM tournament_results \
         WHERE tournament_id = $1 AND final_position = 1 LIMIT 1",
    )
    .bind(tournament_id)
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|r| r.0))
}

/// Settle one prediction: mark won/lost, set payout, and (if won) credit PP.
pub async fn settle_entry(
    pool: &PgPool,
    entry_id: Uuid,
    app_user_id: Uuid,
    won: bool,
    payout_points: i32,
) -> SqlxResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE prediction_entry SET status = $2, payout_points = $3, resolved_at = NOW() \
         WHERE id = $1 AND status = 'open'",
    )
    .bind(entry_id)
    .bind(if won { "won" } else { "lost" })
    .bind(payout_points)
    .execute(&mut *tx)
    .await?;

    if won && payout_points > 0 {
        insert_ledger(
            &mut *tx,
            app_user_id,
            payout_points,
            "prediction_payout",
            Some(entry_id),
        )
        .await?;
    }

    tx.commit().await
}
