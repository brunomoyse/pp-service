use uuid::Uuid;

use infra::repos::{
    club_tables, table_seat_assignments, table_seat_assignments::CreateSeatAssignment,
};

/// Parameters for table balancing (parsed by the resolver).
pub struct BalanceParams {
    pub tournament_id: Uuid,
    pub manager_id: Uuid,
    pub target_players_per_table: Option<i32>,
}

/// Result of a balance operation.
pub struct BalanceResult {
    pub moves: Vec<infra::models::TableSeatAssignmentRow>,
}

/// Check if tables need rebalancing based on player counts.
pub fn needs_rebalancing(table_counts: &std::collections::HashMap<Uuid, usize>) -> bool {
    if table_counts.is_empty() {
        return false;
    }

    let min_count = *table_counts.values().min().unwrap_or(&0);
    let max_count = *table_counts.values().max().unwrap_or(&0);

    // Rebalance if difference is more than 2 players between tables
    // OR if any table has less than 4 players (unless it's the only table)
    (max_count - min_count > 2) || (min_count < 4 && table_counts.len() > 1)
}

/// Perform the table balancing workflow inside a transaction.
///
/// The caller (resolver) is responsible for:
/// - Authentication / authorization
/// - Parsing IDs from GraphQL input
/// - Converting the result to GraphQL types
/// - Publishing subscription events
pub async fn balance_tables(
    pool: &sqlx::PgPool,
    params: BalanceParams,
) -> Result<BalanceResult, Box<dyn std::error::Error + Send + Sync>> {
    // Get all active tables (read before transaction)
    let tables = club_tables::list_assigned_to_tournament(pool, params.tournament_id).await?;
    if tables.is_empty() {
        return Ok(BalanceResult { moves: Vec::new() });
    }

    // Begin transaction
    let mut tx = pool.begin().await?;

    // Get all current assignments within transaction
    let assignments =
        table_seat_assignments::list_current_for_tournament(&mut *tx, params.tournament_id).await?;

    // Group players by table
    let mut table_players: std::collections::HashMap<Uuid, Vec<_>> =
        std::collections::HashMap::new();
    for assignment in assignments {
        table_players
            .entry(assignment.club_table_id)
            .or_default()
            .push(assignment);
    }

    // Count players per table
    let mut table_counts: std::collections::HashMap<Uuid, usize> = std::collections::HashMap::new();
    for (table_id, players) in &table_players {
        table_counts.insert(*table_id, players.len());
    }

    // Check if rebalancing is needed
    if !needs_rebalancing(&table_counts) {
        return Ok(BalanceResult { moves: Vec::new() });
    }

    let total_players = table_counts.values().sum::<usize>();
    let target_per_table = params
        .target_players_per_table
        .unwrap_or(((total_players as f64) / (tables.len() as f64)).ceil() as i32);

    // Find tables that need players and tables that have excess
    let mut need_players: Vec<_> = tables
        .iter()
        .filter(|table| {
            let current_count = table_players.get(&table.id).map(|v| v.len()).unwrap_or(0);
            current_count < target_per_table as usize
        })
        .collect();

    let mut excess_players: Vec<_> = Vec::new();
    for table in &tables {
        let empty_vec = Vec::new();
        let players = table_players.get(&table.id).unwrap_or(&empty_vec);
        if players.len() > target_per_table as usize {
            let excess_count = players.len() - target_per_table as usize;
            let mut sorted_players = players.clone();
            sorted_players.sort_by(|a, b| b.assigned_at.cmp(&a.assigned_at));
            excess_players.extend(sorted_players.into_iter().take(excess_count));
        }
    }

    // Move excess players to tables that need them
    let mut moves = Vec::new();
    for player in excess_players {
        if let Some(target_table) = need_players.first() {
            let current_count = table_players
                .get(&target_table.id)
                .map(|v| v.len())
                .unwrap_or(0);
            if current_count < target_per_table as usize {
                let occupied_seats: std::collections::HashSet<i32> =
                    table_seat_assignments::get_occupied_seats(&mut *tx, target_table.id)
                        .await?
                        .into_iter()
                        .collect();

                let available_seat =
                    (1..=target_table.max_seats).find(|seat| !occupied_seats.contains(seat));

                if let Some(seat_num) = available_seat {
                    table_seat_assignments::unassign_current_seat(
                        &mut *tx,
                        params.tournament_id,
                        player.user_id,
                        Some(params.manager_id),
                    )
                    .await?;

                    let new_assignment = table_seat_assignments::create(
                        &mut *tx,
                        CreateSeatAssignment {
                            tournament_id: params.tournament_id,
                            club_table_id: target_table.id,
                            user_id: player.user_id,
                            seat_number: seat_num,
                            stack_size: player.stack_size,
                            assigned_by: Some(params.manager_id),
                            notes: Some("Balanced by system".to_string()),
                        },
                    )
                    .await?;

                    moves.push(new_assignment.clone());

                    // Update tracking
                    table_players
                        .entry(target_table.id)
                        .or_default()
                        .push(new_assignment);

                    if table_players
                        .get(&target_table.id)
                        .map(|v| v.len())
                        .unwrap_or(0)
                        >= target_per_table as usize
                    {
                        need_players.remove(0);
                    }
                }
            }
        }
    }

    // Commit transaction
    tx.commit().await?;

    Ok(BalanceResult { moves })
}
