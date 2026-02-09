use async_graphql::{dataloader::DataLoader, Context, Object, Result, ID};
use std::collections::HashMap;

use crate::gql::error::ResultExt;
use crate::gql::loaders::TournamentLoader;
use crate::state::AppState;
use infra::repos::{
    tournament_payouts, tournament_results, tournament_results::UserStatistics, tournaments,
};
use uuid::Uuid;

use super::types::{
    CustomPayout, DealType, EnterTournamentResultsInput, EnterTournamentResultsResponse,
    PayoutPosition, PlayerDeal, PlayerStatistics, PlayerStatsResponse, TournamentPayout,
    UserTournamentResult,
};

#[derive(Default)]
pub struct ResultQuery;

#[Object]
impl ResultQuery {
    async fn my_recent_tournament_results(
        &self,
        ctx: &Context<'_>,
        limit: Option<i64>,
    ) -> Result<Vec<UserTournamentResult>> {
        use crate::auth::Claims;

        let claims = ctx
            .data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;

        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let state = ctx.data::<AppState>()?;

        let limit = limit.unwrap_or(10).clamp(1, 50);
        let results = tournament_results::list_user_recent(&state.db, user_id, limit).await?;

        // Collect all tournament IDs and batch load them using DataLoader
        let tournament_ids: Vec<Uuid> = results.iter().map(|r| r.tournament_id).collect();
        let tournament_loader = ctx.data::<DataLoader<TournamentLoader>>()?;
        let tournaments: HashMap<Uuid, _> = tournament_loader
            .load_many(tournament_ids)
            .await
            .gql_err("Data loading failed")?;

        // Build user results by looking up tournaments from the HashMap
        let mut user_results = Vec::new();
        for result_row in results {
            if let Some(tournament_row) = tournaments.get(&result_row.tournament_id) {
                user_results.push(UserTournamentResult {
                    result: result_row.into(),
                    tournament: tournament_row.clone().into(),
                });
            }
        }

        Ok(user_results)
    }

    async fn my_tournament_statistics(&self, ctx: &Context<'_>) -> Result<PlayerStatsResponse> {
        use crate::auth::Claims;

        let claims = ctx
            .data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;

        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let state = ctx.data::<AppState>()?;

        // Get statistics for different time periods
        let stats_7_days = tournament_results::get_user_statistics(&state.db, user_id, 7).await?;
        let stats_30_days = tournament_results::get_user_statistics(&state.db, user_id, 30).await?;
        let stats_year = tournament_results::get_user_statistics(&state.db, user_id, 365).await?;

        // Convert to GraphQL types
        let convert_stats = |stats: UserStatistics| PlayerStatistics {
            total_itm: stats.total_itm,
            total_tournaments: stats.total_tournaments,
            total_winnings: stats.total_winnings,
            total_buy_ins: stats.total_buy_ins,
            itm_percentage: stats.itm_percentage,
            roi_percentage: stats.roi_percentage,
        };

        Ok(PlayerStatsResponse {
            last_7_days: convert_stats(stats_7_days),
            last_30_days: convert_stats(stats_30_days),
            last_year: convert_stats(stats_year),
        })
    }

    /// Get tournament payout structure
    async fn tournament_payout(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<Option<TournamentPayout>> {
        let state = ctx.data::<AppState>()?;

        let tournament_id =
            Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        if let Some(payout_row) =
            tournament_payouts::get_by_tournament(&state.db, tournament_id).await?
        {
            // Parse the JSONB payout_positions into structured data
            let positions_array = payout_row
                .payout_positions
                .as_array()
                .ok_or_else(|| async_graphql::Error::new("Invalid payout positions format"))?;

            let mut positions = Vec::new();
            for pos in positions_array {
                let position = pos
                    .get("position")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| async_graphql::Error::new("Invalid position value"))?
                    as i32;

                let percentage = pos
                    .get("percentage")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| async_graphql::Error::new("Invalid percentage value"))?;

                let amount_cents = pos
                    .get("amount_cents")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| async_graphql::Error::new("Invalid amount_cents value"))?
                    as i32;

                positions.push(PayoutPosition {
                    position,
                    percentage,
                    amount_cents,
                });
            }

            // Sort positions by position number
            positions.sort_by_key(|p| p.position);

            Ok(Some(TournamentPayout {
                id: payout_row.id.into(),
                tournament_id: payout_row.tournament_id.into(),
                template_id: payout_row.template_id.map(|id| id.into()),
                player_count: payout_row.player_count,
                total_prize_pool: payout_row.total_prize_pool,
                positions,
                created_at: payout_row.created_at,
                updated_at: payout_row.updated_at,
            }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Default)]
pub struct ResultMutation;

#[Object]
impl ResultMutation {
    /// Enter tournament results (positions, payouts, and optional deal)
    async fn enter_tournament_results(
        &self,
        ctx: &Context<'_>,
        input: EnterTournamentResultsInput,
    ) -> Result<EnterTournamentResultsResponse> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        // Verify tournament exists and get club_id for auth
        let tournament = tournaments::get_by_id(&state.db, tournament_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        let manager = require_club_manager(ctx, tournament.club_id).await?;
        let manager_id = Uuid::parse_str(manager.id.as_str()).gql_err("Invalid manager ID")?;

        // Delegate to service
        let params = super::service::EnterResultsParams {
            tournament_id,
            manager_id,
            payout_template_id: input.payout_template_id,
            player_positions: input.player_positions,
            deal: input.deal,
        };

        let output = super::service::enter_tournament_results(&state.db, params)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Convert results to GQL types
        let results: Vec<super::types::TournamentResult> =
            output.results.into_iter().map(|r| r.into()).collect();

        // Convert deal to GQL type
        let gql_deal = if let Some(deal_row) = output.deal {
            let custom_payouts = if let Some(payouts_json) = &deal_row.custom_payouts {
                let payouts_obj = payouts_json
                    .as_object()
                    .ok_or_else(|| async_graphql::Error::new("Invalid custom payouts format"))?;

                let mut custom_payouts_vec = Vec::new();
                for (user_id, amount) in payouts_obj {
                    let amount_cents = amount
                        .as_i64()
                        .ok_or_else(|| async_graphql::Error::new("Invalid payout amount"))?
                        as i32;
                    custom_payouts_vec.push(CustomPayout {
                        user_id: user_id.clone().into(),
                        amount_cents,
                    });
                }
                Some(custom_payouts_vec)
            } else {
                None
            };

            let deal_type = match deal_row.deal_type.as_str() {
                "even_split" => DealType::EvenSplit,
                "icm" => DealType::Icm,
                "custom" => DealType::Custom,
                _ => DealType::EvenSplit,
            };

            Some(PlayerDeal {
                id: deal_row.id.into(),
                tournament_id: deal_row.tournament_id.into(),
                deal_type,
                affected_positions: deal_row.affected_positions,
                custom_payouts,
                total_amount_cents: deal_row.total_amount_cents,
                notes: deal_row.notes,
                created_by: deal_row.created_by.into(),
            })
        } else {
            None
        };

        Ok(EnterTournamentResultsResponse {
            success: true,
            results,
            deal: gql_deal,
        })
    }
}
