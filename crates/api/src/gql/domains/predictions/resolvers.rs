use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::auth::permissions::require_club_manager;
use crate::features::{require_feature, Feature};
use crate::gql::common::helpers::get_club_id_for_tournament;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::predictions;

use super::service;
use super::types::{PredictionBalance, PredictionEntry};

fn current_user_id(ctx: &Context<'_>) -> Result<Uuid> {
    let claims = ctx.data::<Claims>()?;
    Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")
}

#[derive(Default)]
pub struct PredictionsQuery;

#[Object]
impl PredictionsQuery {
    /// The current user's Prediction-Points balance and what they can claim.
    async fn my_prediction_balance(&self, ctx: &Context<'_>) -> Result<PredictionBalance> {
        require_feature(Feature::Predictions)?;
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let balance = predictions::balance(&state.db, user_id).await?;
        let claimable = service::claimable(&state.db, user_id).await?;
        Ok(PredictionBalance {
            balance: balance as i32,
            claimable: claimable as i32,
        })
    }

    /// The current user's fantasy predictions, newest first.
    async fn my_predictions(&self, ctx: &Context<'_>) -> Result<Vec<PredictionEntry>> {
        require_feature(Feature::Predictions)?;
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let rows = predictions::list_for_user(&state.db, user_id).await?;
        Ok(rows.into_iter().map(PredictionEntry::from).collect())
    }
}

#[derive(Default)]
pub struct PredictionsMutation;

#[Object]
impl PredictionsMutation {
    /// Claim earned prediction points (from attendance/play) plus the one-time
    /// welcome seed. Earned-only — no euros ever enter this balance (G2).
    async fn claim_prediction_points(&self, ctx: &Context<'_>) -> Result<PredictionBalance> {
        require_feature(Feature::Predictions)?;
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let balance = service::claim(&state.db, user_id).await?;
        Ok(PredictionBalance {
            balance: balance as i32,
            claimable: 0,
        })
    }

    /// Place a free fantasy pick on a tournament winner, staking prediction points.
    async fn create_prediction(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
        predicted_winner_user_id: ID,
        stake_points: i32,
    ) -> Result<PredictionEntry> {
        require_feature(Feature::Predictions)?;
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let tid = Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;
        let winner =
            Uuid::parse_str(predicted_winner_user_id.as_str()).gql_err("Invalid user ID")?;

        let entry =
            service::create_prediction(&state.db, user_id, tid, winner, stake_points).await?;

        // Re-read with names for a complete payload.
        let view = predictions::list_for_user(&state.db, user_id)
            .await?
            .into_iter()
            .find(|v| v.id == entry.id)
            .ok_or_else(|| async_graphql::Error::new("Prediction not found"))?;
        Ok(PredictionEntry::from(view))
    }

    /// Settle every open prediction for a finished tournament. The tournament's
    /// club managers only. Returns how many were resolved.
    async fn resolve_tournament_predictions(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<i32> {
        require_feature(Feature::Predictions)?;
        let state = ctx.data::<AppState>()?;
        let tid = Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let club_id = get_club_id_for_tournament(&state.db, tid).await?;
        require_club_manager(ctx, club_id).await?;

        Ok(service::resolve_tournament(&state.db, tid).await?)
    }
}
