use uuid::Uuid;

use crate::gql::error::GqlError;
use infra::db::Db;
use infra::repos::predictions;

/// PP earned per lifetime check-in (ties PP to attendance — never euros, G2).
const CHECK_IN_PP: i64 = 25;
/// PP earned per tournament played.
const TOURNAMENT_PP: i64 = 50;
/// One-time welcome grant so new players can join the fantasy game.
const SEED_PP: i32 = 500;
/// Winners are paid this multiple of their stake.
const PAYOUT_MULTIPLIER: i32 = 2;

/// Total PP a player has earned from activity (before subtracting what's claimed).
async fn earned_entitlement(db: &Db, user_id: Uuid) -> Result<i64, GqlError> {
    let check_ins = predictions::check_in_total(db, user_id).await?;
    let tournaments = predictions::tournaments_total(db, user_id).await?;
    Ok(check_ins * CHECK_IN_PP + tournaments * TOURNAMENT_PP)
}

/// Points claimable right now: unclaimed earned activity + the first-time seed.
pub async fn claimable(db: &Db, user_id: Uuid) -> Result<i64, GqlError> {
    let entitlement = earned_entitlement(db, user_id).await?;
    let already = predictions::credited_for_reason(db, user_id, "earned").await?;
    let seed = if predictions::has_reason(db, user_id, "seed").await? {
        0
    } else {
        SEED_PP as i64
    };
    Ok((entitlement - already).max(0) + seed)
}

/// Credit any unclaimed earned points (+ first-time seed). Returns new balance.
pub async fn claim(db: &Db, user_id: Uuid) -> Result<i64, GqlError> {
    if !predictions::has_reason(db, user_id, "seed").await? {
        predictions::insert_ledger(db, user_id, SEED_PP, "seed", None).await?;
    }
    let entitlement = earned_entitlement(db, user_id).await?;
    let already = predictions::credited_for_reason(db, user_id, "earned").await?;
    let diff = (entitlement - already).max(0);
    if diff > 0 {
        predictions::insert_ledger(db, user_id, diff as i32, "earned", None).await?;
    }
    Ok(predictions::balance(db, user_id).await?)
}

/// Place a fantasy pick. Predictions close once results are in (a winner exists).
pub async fn create_prediction(
    db: &Db,
    user_id: Uuid,
    tournament_id: Uuid,
    predicted_winner_user_id: Uuid,
    stake_points: i32,
) -> Result<infra::models::PredictionEntryRow, GqlError> {
    if stake_points <= 0 {
        return Err(GqlError::new("Stake must be positive"));
    }
    if predictions::winner_user_id(db, tournament_id)
        .await?
        .is_some()
    {
        return Err(GqlError::new("Predictions are closed for this tournament"));
    }
    if predictions::get_entry(db, user_id, tournament_id)
        .await?
        .is_some()
    {
        return Err(GqlError::new(
            "You already have a prediction for this tournament",
        ));
    }
    let balance = predictions::balance(db, user_id).await?;
    if balance < stake_points as i64 {
        return Err(GqlError::new("Not enough prediction points"));
    }

    Ok(predictions::create_entry(
        db,
        user_id,
        tournament_id,
        predicted_winner_user_id,
        stake_points,
    )
    .await?)
}

/// Settle every open prediction for a finished tournament. Returns how many were
/// resolved. Winners earn `stake * PAYOUT_MULTIPLIER`.
pub async fn resolve_tournament(db: &Db, tournament_id: Uuid) -> Result<i32, GqlError> {
    let winner = predictions::winner_user_id(db, tournament_id)
        .await?
        .ok_or_else(|| GqlError::new("No winner recorded yet for this tournament"))?;

    let entries = predictions::open_for_tournament(db, tournament_id).await?;
    let mut resolved = 0;
    for entry in entries {
        let won = entry.predicted_winner_user_id == winner;
        let payout = if won {
            entry.stake_points * PAYOUT_MULTIPLIER
        } else {
            0
        };
        predictions::settle_entry(db, entry.id, entry.app_user_id, won, payout).await?;
        resolved += 1;
    }
    Ok(resolved)
}
