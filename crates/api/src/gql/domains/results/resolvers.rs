use async_graphql::{dataloader::DataLoader, Context, Object, Result, ID};
use std::collections::HashMap;

use crate::gql::error::ResultExt;
use crate::gql::loaders::TournamentLoader;
use crate::gql::types::Tournament;
use crate::state::AppState;
use infra::models::TournamentRow;
use infra::repos::{
    payout_templates, player_deals, player_deals::CreatePlayerDeal, tournament_payouts,
    tournament_results, tournament_results::UserStatistics, tournaments,
};
use uuid::Uuid;

use super::types::{
    CustomPayout, DealType, EnterTournamentResultsInput, EnterTournamentResultsResponse,
    PayoutPosition, PlayerDeal, PlayerDealInput, PlayerStatistics, PlayerStatsResponse,
    TournamentPayout, TournamentResult, UserTournamentResult,
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
                let tournament_result = TournamentResult {
                    id: result_row.id.into(),
                    tournament_id: result_row.tournament_id.into(),
                    user_id: result_row.user_id.into(),
                    final_position: result_row.final_position,
                    prize_cents: result_row.prize_cents,
                    points: result_row.points,
                    notes: result_row.notes.clone(),
                    created_at: result_row.created_at,
                };

                let status = tournament_row.calculate_status().into();
                let tournament = Tournament {
                    id: tournament_row.id.into(),
                    title: tournament_row.name.clone(),
                    description: tournament_row.description.clone(),
                    club_id: tournament_row.club_id.into(),
                    start_time: tournament_row.start_time,
                    end_time: tournament_row.end_time,
                    buy_in_cents: tournament_row.buy_in_cents,
                    seat_cap: tournament_row.seat_cap,
                    status,
                    live_status: tournament_row.live_status.into(),
                    early_bird_bonus_chips: tournament_row.early_bird_bonus_chips,
                    created_at: tournament_row.created_at,
                    updated_at: tournament_row.updated_at,
                };

                user_results.push(UserTournamentResult {
                    result: tournament_result,
                    tournament,
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
        use crate::auth::permissions::require_role;
        use crate::gql::types::Role as GqlRole;
        use infra::repos::tournament_results::CreateTournamentResult;

        // Require manager role
        let manager = require_role(ctx, GqlRole::Manager).await?;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        // Verify tournament exists
        let tournament = tournaments::get_by_id(&state.db, tournament_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        // Calculate payouts
        let total_prize_pool =
            calculate_prize_pool(&tournament, input.player_positions.len() as i32)?;
        let payouts = calculate_payouts(
            &state.db,
            input.payout_template_id.as_ref(),
            &input.player_positions,
            total_prize_pool,
            input.deal.as_ref(),
        )
        .await?;

        // Begin transaction for all write operations
        let mut tx = state
            .db
            .begin()
            .await
            .gql_err("Database operation failed")?;

        // Create player deal if specified
        let deal = if let Some(deal_input) = input.deal {
            let manager_id = Uuid::parse_str(manager.id.as_str()).gql_err("Invalid manager ID")?;

            let custom_payouts = if let Some(custom) = &deal_input.custom_payouts {
                let mut payouts_map = serde_json::Map::new();
                for payout in custom {
                    payouts_map.insert(
                        payout.user_id.to_string(),
                        serde_json::Value::Number(serde_json::Number::from(payout.amount_cents)),
                    );
                }
                Some(serde_json::Value::Object(payouts_map))
            } else {
                None
            };

            let total_deal_amount = calculate_deal_total(&deal_input, &payouts)?;

            let deal_data = CreatePlayerDeal {
                tournament_id,
                deal_type: match deal_input.deal_type {
                    DealType::EvenSplit => "even_split".to_string(),
                    DealType::Icm => "icm".to_string(),
                    DealType::Custom => "custom".to_string(),
                },
                affected_positions: deal_input.affected_positions.clone(),
                custom_payouts,
                total_amount_cents: total_deal_amount,
                notes: deal_input.notes.clone(),
                created_by: manager_id,
            };

            Some(
                player_deals::create(&mut *tx, deal_data)
                    .await
                    .gql_err("Database operation failed")?,
            )
        } else {
            None
        };

        // Create tournament results
        let mut results = Vec::new();
        for (position_input, payout_amount) in input.player_positions.iter().zip(payouts.iter()) {
            let user_id =
                Uuid::parse_str(position_input.user_id.as_str()).gql_err("Invalid user ID")?;

            let result_data = CreateTournamentResult {
                tournament_id,
                user_id,
                final_position: position_input.final_position,
                prize_cents: *payout_amount,
                notes: None,
            };

            let result_row = tournament_results::create(&mut *tx, result_data)
                .await
                .gql_err("Database operation failed")?;
            results.push(TournamentResult {
                id: result_row.id.into(),
                tournament_id: result_row.tournament_id.into(),
                user_id: result_row.user_id.into(),
                final_position: result_row.final_position,
                prize_cents: result_row.prize_cents,
                points: result_row.points,
                notes: result_row.notes,
                created_at: result_row.created_at,
            });
        }

        // Commit transaction
        tx.commit().await.gql_err("Database operation failed")?;

        // Convert deal to GraphQL type
        let gql_deal = if let Some(deal_row) = deal {
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

// Helper functions for payout calculation

fn calculate_prize_pool(tournament: &TournamentRow, player_count: i32) -> Result<i32> {
    let total_pool = tournament.buy_in_cents * player_count;
    Ok(total_pool)
}

async fn calculate_payouts(
    db: &infra::db::Db,
    template_id: Option<&ID>,
    positions: &[super::types::PlayerPositionInput],
    total_prize_pool: i32,
    deal: Option<&PlayerDealInput>,
) -> Result<Vec<i32>> {
    let mut payouts = vec![0; positions.len()];

    // If there's a deal that affects certain positions, handle it
    if let Some(deal_input) = deal {
        match deal_input.deal_type {
            DealType::EvenSplit => {
                let affected_total = if let Some(template_id) = template_id {
                    calculate_template_total(
                        db,
                        template_id,
                        &deal_input.affected_positions,
                        total_prize_pool,
                    )
                    .await?
                } else {
                    total_prize_pool
                };

                let per_player = affected_total / deal_input.affected_positions.len() as i32;

                for position in positions {
                    if deal_input
                        .affected_positions
                        .contains(&position.final_position)
                    {
                        let index = (position.final_position - 1) as usize;
                        if index < payouts.len() {
                            payouts[index] = per_player;
                        }
                    }
                }
            }
            DealType::Custom => {
                if let Some(custom_payouts) = &deal_input.custom_payouts {
                    for position in positions {
                        if deal_input
                            .affected_positions
                            .contains(&position.final_position)
                        {
                            for custom in custom_payouts {
                                if custom.user_id == position.user_id {
                                    let index = (position.final_position - 1) as usize;
                                    if index < payouts.len() {
                                        payouts[index] = custom.amount_cents;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            DealType::Icm => {
                let affected_total = if let Some(template_id) = template_id {
                    calculate_template_total(
                        db,
                        template_id,
                        &deal_input.affected_positions,
                        total_prize_pool,
                    )
                    .await?
                } else {
                    total_prize_pool
                };

                let per_player = affected_total / deal_input.affected_positions.len() as i32;

                for position in positions {
                    if deal_input
                        .affected_positions
                        .contains(&position.final_position)
                    {
                        let index = (position.final_position - 1) as usize;
                        if index < payouts.len() {
                            payouts[index] = per_player;
                        }
                    }
                }
            }
        }

        // Calculate remaining positions using template if available
        if let Some(template_id) = template_id {
            let template_id_uuid =
                Uuid::parse_str(template_id.as_str()).gql_err("Invalid template ID")?;

            if let Some(template) = payout_templates::get_by_id(db, template_id_uuid).await? {
                let payout_structure = parse_payout_structure(&template.payout_structure)?;

                for position in positions {
                    if !deal_input
                        .affected_positions
                        .contains(&position.final_position)
                    {
                        if let Some(percentage) =
                            get_position_percentage(&payout_structure, position.final_position)
                        {
                            let index = (position.final_position - 1) as usize;
                            if index < payouts.len() {
                                payouts[index] =
                                    ((total_prize_pool as f64 * percentage / 100.0).round()) as i32;
                            }
                        }
                    }
                }
            }
        }
    } else {
        // No deal - use template for all positions
        if let Some(template_id) = template_id {
            let template_id_uuid =
                Uuid::parse_str(template_id.as_str()).gql_err("Invalid template ID")?;

            if let Some(template) = payout_templates::get_by_id(db, template_id_uuid).await? {
                let payout_structure = parse_payout_structure(&template.payout_structure)?;

                for position in positions {
                    if let Some(percentage) =
                        get_position_percentage(&payout_structure, position.final_position)
                    {
                        let index = (position.final_position - 1) as usize;
                        if index < payouts.len() {
                            payouts[index] =
                                ((total_prize_pool as f64 * percentage / 100.0).round()) as i32;
                        }
                    }
                }
            }
        }
    }

    // Adjust for rounding remainder
    let payout_sum: i32 = payouts.iter().sum();
    let remainder = total_prize_pool - payout_sum;
    if remainder != 0 {
        if let Some(last_paid) = payouts.iter().rposition(|&p| p > 0) {
            payouts[last_paid] += remainder;
        }
    }

    Ok(payouts)
}

async fn calculate_template_total(
    db: &infra::db::Db,
    template_id: &ID,
    affected_positions: &[i32],
    total_prize_pool: i32,
) -> Result<i32> {
    let template_id_uuid = Uuid::parse_str(template_id.as_str()).gql_err("Invalid template ID")?;

    if let Some(template) = payout_templates::get_by_id(db, template_id_uuid).await? {
        let payout_structure = parse_payout_structure(&template.payout_structure)?;
        let mut total_percentage = 0.0;

        for position in affected_positions {
            if let Some(percentage) = get_position_percentage(&payout_structure, *position) {
                total_percentage += percentage;
            }
        }

        Ok(((total_prize_pool as f64 * total_percentage / 100.0).round()) as i32)
    } else {
        Ok(total_prize_pool)
    }
}

fn calculate_deal_total(deal_input: &PlayerDealInput, payouts: &[i32]) -> Result<i32> {
    match deal_input.deal_type {
        DealType::Custom => {
            if let Some(custom_payouts) = &deal_input.custom_payouts {
                Ok(custom_payouts.iter().map(|p| p.amount_cents).sum())
            } else {
                Ok(0)
            }
        }
        _ => {
            let mut total = 0;
            for position in &deal_input.affected_positions {
                let index = (*position - 1) as usize;
                if index < payouts.len() {
                    total += payouts[index];
                }
            }
            Ok(total)
        }
    }
}

fn parse_payout_structure(structure: &serde_json::Value) -> Result<Vec<(i32, f64)>> {
    let array = structure
        .as_array()
        .ok_or_else(|| async_graphql::Error::new("Invalid payout structure format"))?;

    let mut payouts = Vec::new();
    for item in array {
        let obj = item
            .as_object()
            .ok_or_else(|| async_graphql::Error::new("Invalid payout item format"))?;

        let position = obj
            .get("position")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| async_graphql::Error::new("Missing or invalid position"))?
            as i32;

        let percentage = obj
            .get("percentage")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| async_graphql::Error::new("Missing or invalid percentage"))?;

        if !(0.0..=100.0).contains(&percentage) {
            return Err(async_graphql::Error::new(
                "Each payout percentage must be between 0 and 100",
            ));
        }

        payouts.push((position, percentage));
    }

    let total: f64 = payouts.iter().map(|(_, p)| p).sum();
    if (total - 100.0).abs() > 0.01 {
        return Err(async_graphql::Error::new(format!(
            "Payout percentages must sum to 100%, got {:.2}%",
            total
        )));
    }

    Ok(payouts)
}

fn get_position_percentage(payout_structure: &[(i32, f64)], position: i32) -> Option<f64> {
    payout_structure
        .iter()
        .find(|(pos, _)| *pos == position)
        .map(|(_, percentage)| *percentage)
}
