use async_graphql::ID;
use uuid::Uuid;

use infra::repos::{
    payout_templates, player_deals, player_deals::CreatePlayerDeal, tournament_payouts,
    tournament_results, tournament_results::CreateTournamentResult, tournaments,
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
    pub newly_unlocked_achievements: Vec<(
        Uuid,
        crate::gql::domains::achievements::service::UnlockedAchievement,
    )>,
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
    let _tournament = tournaments::get_by_id(pool, params.tournament_id)
        .await?
        .ok_or("Tournament not found")?;

    // The prize pool is maintained by the DB trigger
    // recalculate_prize_pool_from_entries (sum of entries minus the bounty slice,
    // excluding voucher/bonus). Use it as the single source of truth so payouts
    // reconcile with the cash report and PKO accounting, instead of re-deriving
    // buy_in × players (which ignored rebuys, add-ons, and the bounty slice).
    // No payouts row means no entries were recorded, hence no pool (0) — never
    // fabricate money for un-recorded buy-ins.
    let total_prize_pool = tournament_payouts::get_by_tournament(pool, params.tournament_id)
        .await?
        .map(|p| p.total_prize_pool)
        .unwrap_or(0);

    let payouts = calculate_payouts(
        pool,
        params.payout_template_id.as_ref(),
        &params.player_positions,
        total_prize_pool,
        params.deal.as_ref(),
    )
    .await?;

    // Reconciliation guard: distributed payouts must sum to the prize pool.
    // calculate_payouts already dumps any rounding remainder on the last paid
    // position, so this mainly catches a non-zero pool with nothing paid out
    // (e.g. a missing/empty payout template) before we persist anything.
    let payout_total: i32 = payouts.iter().sum();
    if payout_total != total_prize_pool {
        return Err(format!(
            "Payout reconciliation failed: distributed {payout_total} cents but prize pool is {total_prize_pool} cents"
        )
        .into());
    }

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
            user_id: Some(user_id),
            club_player_id: None,
            final_position: position_input.final_position,
            prize_cents: *payout_amount,
            notes: None,
        };

        let result_row = tournament_results::create(&mut *tx, result_data).await?;
        results.push(result_row);
    }

    // Entering final results ends the tournament. Do it inside the same
    // transaction so the results, the (optional) deal, and the FINISHED
    // transition commit atomically — a manager can never end up with recorded
    // results on a tournament still stuck at FINAL_TABLE.
    tournaments::update_live_status(
        &mut *tx,
        params.tournament_id,
        tournaments::TournamentLiveStatus::Finished,
    )
    .await?;

    // Evaluate achievements for each player
    let mut newly_unlocked_achievements: Vec<(
        Uuid,
        crate::gql::domains::achievements::service::UnlockedAchievement,
    )> = Vec::new();
    for result in &results {
        // Achievements only apply to app users; account-less players have no user_id.
        let Some(user_id) = result.user_id else {
            continue;
        };
        match crate::gql::domains::achievements::service::evaluate_for_player(
            &mut tx,
            user_id,
            params.tournament_id,
        )
        .await
        {
            Ok(unlocked) => {
                for achievement in unlocked {
                    newly_unlocked_achievements.push((user_id, achievement));
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to evaluate achievements for user {}: {}",
                    user_id,
                    e
                );
            }
        }
    }

    // Commit transaction
    tx.commit().await?;

    Ok(EnterResultsOutput {
        results,
        deal,
        newly_unlocked_achievements,
    })
}

// --- Private helpers ---

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
                // Real chip-based ICM when requested and the inputs allow it
                // (template for the place prizes + a positive stack per player).
                // Otherwise fall through to an even split — including as the ICM
                // fallback, so a deal always produces a valid payout.
                let icm = if matches!(deal_input.deal_type, DealType::Icm) {
                    calculate_icm_payouts(db, template_id, positions, deal_input, total_prize_pool)
                        .await?
                } else {
                    None
                };

                if let Some(icm_payouts) = icm {
                    for (index, amount) in icm_payouts {
                        if index < payouts.len() {
                            payouts[index] = amount;
                        }
                    }
                } else {
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

/// Compute an ICM (Independent Chip Model) split for a deal, returning
/// `(payout_index, amount_cents)` for each player covered by the deal.
///
/// The money at stake is the sum of the template prizes for the affected
/// finishing places; ICM shares it by each player's chip stack (their
/// probability of finishing in each remaining place — Malmuth-Harville).
/// Payouts are rounded to whole euros (largest-remainder over the whole-euro
/// pool) so players never receive eurocents; any sub-euro remainder from a
/// pool that isn't a round number of euros lands on the chip leader. The
/// returned amounts always sum back to the affected total so the caller's
/// reconciliation guard holds.
///
/// Returns `None` (→ caller falls back to an even split) when a template or
/// chip stacks are missing, a stack is non-positive, or the affected prizes
/// are zero — i.e. whenever ICM is undefined.
async fn calculate_icm_payouts(
    db: &sqlx::PgPool,
    template_id: Option<&ID>,
    positions: &[PlayerPositionInput],
    deal_input: &PlayerDealInput,
    total_prize_pool: i32,
) -> Result<Option<Vec<(usize, i32)>>, Box<dyn std::error::Error + Send + Sync>> {
    let Some(template_id) = template_id else {
        return Ok(None);
    };
    let Some(chip_counts) = deal_input.chip_counts.as_ref() else {
        return Ok(None);
    };
    if chip_counts.is_empty() {
        return Ok(None);
    }

    let template_id_uuid =
        Uuid::parse_str(template_id.as_str()).map_err(|_| "Invalid template ID")?;
    let Some(template) = payout_templates::get_by_id(db, template_id_uuid).await? else {
        return Ok(None);
    };
    let payout_structure = parse_payout_structure(&template.payout_structure)?;

    let chips_by_user: std::collections::HashMap<&str, i32> = chip_counts
        .iter()
        .map(|c| (c.user_id.as_str(), c.chips))
        .collect();

    // Players in the deal, paired with their payout slot index and stack.
    let mut players: Vec<(usize, f64)> = Vec::new();
    for position in positions {
        if deal_input
            .affected_positions
            .contains(&position.final_position)
        {
            let index = (position.final_position - 1) as usize;
            let chips = *chips_by_user.get(position.user_id.as_str()).unwrap_or(&0);
            players.push((index, chips as f64));
        }
    }
    if players.is_empty() || players.iter().any(|(_, stack)| *stack <= 0.0) {
        return Ok(None);
    }

    // Prize for each affected finishing place, largest first — ICM assigns the
    // top remaining prize to whoever "wins" at each level of the recursion.
    let mut place_prizes: Vec<i32> = deal_input
        .affected_positions
        .iter()
        .map(|p| {
            get_position_percentage(&payout_structure, *p)
                .map(|pct| (total_prize_pool as f64 * pct / 100.0).round() as i32)
                .unwrap_or(0)
        })
        .collect();
    place_prizes.sort_unstable_by(|a, b| b.cmp(a));

    let affected_total: i32 = place_prizes.iter().sum();
    if affected_total <= 0 {
        return Ok(None);
    }

    let stacks: Vec<f64> = players.iter().map(|(_, stack)| *stack).collect();
    let prizes: Vec<f64> = place_prizes.iter().map(|&p| p as f64).collect();
    let equities = icm_equity(&stacks, &prizes);

    // Round to whole euros while conserving the exact affected total.
    let n = players.len();
    let euros_total = affected_total / 100;
    let sub_euro = affected_total - euros_total * 100;
    let raw_euros: Vec<f64> = equities.iter().map(|cents| cents / 100.0).collect();
    let mut floor_euros: Vec<i32> = raw_euros.iter().map(|e| e.floor() as i32).collect();
    let assigned: i32 = floor_euros.iter().sum();

    // Hand out the remaining whole euros to the largest fractional parts.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        let frac = |i: usize| raw_euros[i] - floor_euros[i] as f64;
        frac(b)
            .partial_cmp(&frac(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut leftover = euros_total - assigned;
    let mut k = 0usize;
    while leftover > 0 {
        floor_euros[order[k % n]] += 1;
        leftover -= 1;
        k += 1;
    }

    let mut cents: Vec<i32> = floor_euros.iter().map(|e| e * 100).collect();
    if sub_euro != 0 {
        let leader = stacks
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);
        cents[leader] += sub_euro;
    }

    Ok(Some(
        players
            .iter()
            .enumerate()
            .map(|(i, (index, _))| (*index, cents[i]))
            .collect(),
    ))
}

/// Malmuth-Harville ICM equities: expected prize per player given chip stacks
/// and the (descending) prize for each remaining place. `n <= 9` at a final
/// table, so the factorial recursion is trivially cheap.
fn icm_equity(stacks: &[f64], prizes: &[f64]) -> Vec<f64> {
    let n = stacks.len();
    let mut equity = vec![0.0; n];
    if prizes.is_empty() || n == 0 {
        return equity;
    }
    let total: f64 = stacks.iter().sum();
    if total <= 0.0 {
        return equity;
    }

    let prize = prizes[0];
    let rest = &prizes[1..];
    for i in 0..n {
        let p_first = stacks[i] / total;
        equity[i] += p_first * prize;

        if !rest.is_empty() && n > 1 {
            let sub_stacks: Vec<f64> = (0..n).filter(|&j| j != i).map(|j| stacks[j]).collect();
            let sub_equity = icm_equity(&sub_stacks, rest);
            let mut k = 0;
            for (j, eq) in equity.iter_mut().enumerate() {
                if j == i {
                    continue;
                }
                *eq += p_first * sub_equity[k];
                k += 1;
            }
        }
    }
    equity
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
