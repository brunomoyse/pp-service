use async_graphql::ID;
use uuid::Uuid;

use infra::repos::{
    payout_templates, player_deals, player_deals::CreatePlayerDeal, tournament_results,
    tournament_results::CreateTournamentResult, tournaments,
};

use super::types::{DealType, PlayerDealInput, PlayerPositionInput};

/// Parameters for entering tournament results (parsed by the resolver).
pub struct EnterResultsParams {
    pub tournament_id: Uuid,
    pub manager_id: Uuid,
    pub payout_template_id: Option<ID>,
    pub player_positions: Vec<PlayerPositionInput>,
    pub deal: Option<PlayerDealInput>,
}

/// Output of the enter-results workflow.
pub struct EnterResultsOutput {
    pub results: Vec<infra::models::TournamentResultRow>,
    pub deal: Option<infra::models::PlayerDealRow>,
}

/// Enter tournament results inside a transaction.
///
/// The caller (resolver) is responsible for:
/// - Authentication / authorization
/// - Parsing IDs from GraphQL input
/// - Converting the output to GraphQL types
pub async fn enter_tournament_results(
    pool: &sqlx::PgPool,
    params: EnterResultsParams,
) -> Result<EnterResultsOutput, Box<dyn std::error::Error + Send + Sync>> {
    // Verify tournament exists
    let tournament = tournaments::get_by_id(pool, params.tournament_id)
        .await?
        .ok_or("Tournament not found")?;

    // Calculate payouts
    let total_prize_pool = calculate_prize_pool(&tournament, params.player_positions.len() as i32);
    let payouts = calculate_payouts(
        pool,
        params.payout_template_id.as_ref(),
        &params.player_positions,
        total_prize_pool,
        params.deal.as_ref(),
    )
    .await?;

    // Begin transaction
    let mut tx = pool.begin().await?;

    // Create player deal if specified
    let deal = if let Some(deal_input) = &params.deal {
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

        let total_deal_amount = calculate_deal_total(deal_input, &payouts);

        let deal_data = CreatePlayerDeal {
            tournament_id: params.tournament_id,
            deal_type: match deal_input.deal_type {
                DealType::EvenSplit => "even_split".to_string(),
                DealType::Icm => "icm".to_string(),
                DealType::Custom => "custom".to_string(),
            },
            affected_positions: deal_input.affected_positions.clone(),
            custom_payouts,
            total_amount_cents: total_deal_amount,
            notes: deal_input.notes.clone(),
            created_by: params.manager_id,
        };

        Some(player_deals::create(&mut *tx, deal_data).await?)
    } else {
        None
    };

    // Create tournament results
    let mut results = Vec::new();
    for (position_input, payout_amount) in params.player_positions.iter().zip(payouts.iter()) {
        let user_id =
            Uuid::parse_str(position_input.user_id.as_str()).map_err(|_| "Invalid user ID")?;

        let result_data = CreateTournamentResult {
            tournament_id: params.tournament_id,
            user_id,
            final_position: position_input.final_position,
            prize_cents: *payout_amount,
            notes: None,
        };

        let result_row = tournament_results::create(&mut *tx, result_data).await?;
        results.push(result_row);
    }

    // Commit transaction
    tx.commit().await?;

    Ok(EnterResultsOutput { results, deal })
}

// --- Private helpers ---

fn calculate_prize_pool(tournament: &infra::models::TournamentRow, player_count: i32) -> i32 {
    tournament.buy_in_cents * player_count
}

async fn calculate_payouts(
    db: &sqlx::PgPool,
    template_id: Option<&ID>,
    positions: &[PlayerPositionInput],
    total_prize_pool: i32,
    deal: Option<&PlayerDealInput>,
) -> Result<Vec<i32>, Box<dyn std::error::Error + Send + Sync>> {
    let mut payouts = vec![0; positions.len()];

    if let Some(deal_input) = deal {
        match deal_input.deal_type {
            DealType::EvenSplit | DealType::Icm => {
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
        }

        // Calculate remaining positions using template
        if let Some(template_id) = template_id {
            let template_id_uuid =
                Uuid::parse_str(template_id.as_str()).map_err(|_| "Invalid template ID")?;

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
                Uuid::parse_str(template_id.as_str()).map_err(|_| "Invalid template ID")?;

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
    db: &sqlx::PgPool,
    template_id: &ID,
    affected_positions: &[i32],
    total_prize_pool: i32,
) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    let template_id_uuid =
        Uuid::parse_str(template_id.as_str()).map_err(|_| "Invalid template ID")?;

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

fn calculate_deal_total(deal_input: &PlayerDealInput, payouts: &[i32]) -> i32 {
    match deal_input.deal_type {
        DealType::Custom => {
            if let Some(custom_payouts) = &deal_input.custom_payouts {
                custom_payouts.iter().map(|p| p.amount_cents).sum()
            } else {
                0
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
            total
        }
    }
}

fn parse_payout_structure(
    structure: &serde_json::Value,
) -> Result<Vec<(i32, f64)>, Box<dyn std::error::Error + Send + Sync>> {
    let array = structure
        .as_array()
        .ok_or("Invalid payout structure format")?;

    let mut payouts = Vec::new();
    for item in array {
        let obj = item.as_object().ok_or("Invalid payout item format")?;

        let position = obj
            .get("position")
            .and_then(|v| v.as_i64())
            .ok_or("Missing or invalid position")? as i32;

        let percentage = obj
            .get("percentage")
            .and_then(|v| v.as_f64())
            .ok_or("Missing or invalid percentage")?;

        if !(0.0..=100.0).contains(&percentage) {
            return Err("Each payout percentage must be between 0 and 100".into());
        }

        payouts.push((position, percentage));
    }

    let total: f64 = payouts.iter().map(|(_, p)| p).sum();
    if (total - 100.0).abs() > 0.01 {
        return Err(format!("Payout percentages must sum to 100%, got {:.2}%", total).into());
    }

    Ok(payouts)
}

fn get_position_percentage(payout_structure: &[(i32, f64)], position: i32) -> Option<f64> {
    payout_structure
        .iter()
        .find(|(pos, _)| *pos == position)
        .map(|(_, percentage)| *percentage)
}
