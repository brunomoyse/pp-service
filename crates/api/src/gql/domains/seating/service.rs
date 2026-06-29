use std::collections::{HashMap, HashSet};

use rand::seq::SliceRandom;
use uuid::Uuid;

use infra::repos::{
    club_tables, table_seat_assignments, table_seat_assignments::CreateSeatAssignment,
    tournament_registrations,
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

/// Result of an auto-seat operation.
pub struct AutoSeatResult {
    pub assignments: Vec<infra::models::TableSeatAssignmentRow>,
}

/// Working state for one table while filling free seats.
struct TableFill {
    id: Uuid,
    max_seats: i32,
    occupied: HashSet<i32>,
}

/// One decided seat placement, produced by the (synchronous) draw before any
/// database writes.
struct SeatPlan {
    club_player_id: Uuid,
    user_id: Option<Uuid>,
    club_table_id: Uuid,
    seat_number: i32,
    stack_size: Option<i32>,
}

/// Pick the least-filled table that still has a free seat and reserve a random
/// free seat on it (mutating its `occupied` set). Returns the table id and seat
/// number, or `None` when every table is full. Synchronous so the RNG never
/// crosses an await point.
fn pick_random_seat(fills: &mut [TableFill], rng: &mut impl rand::RngExt) -> Option<(Uuid, i32)> {
    let table = fills
        .iter_mut()
        .filter(|f| (f.occupied.len() as i32) < f.max_seats)
        .min_by_key(|f| f.occupied.len())?;
    let free: Vec<i32> = (1..=table.max_seats)
        .filter(|n| !table.occupied.contains(n))
        .collect();
    let seat_number = free[rng.random_range(0..free.len())];
    table.occupied.insert(seat_number);
    Some((table.id, seat_number))
}

/// Seat one checked-in player on a random free seat across the linked tables
/// (least-filled table first), moving their registration to SEATED. Returns
/// `None` when there are no tables, no free seat, or the player already holds a
/// seat. Pure DB work; the caller publishes the seating event and logs.
pub async fn auto_seat_one(
    pool: &sqlx::PgPool,
    tournament_id: Uuid,
    club_player_id: Uuid,
    manager_id: Uuid,
) -> Result<Option<infra::models::TableSeatAssignmentRow>, Box<dyn std::error::Error + Send + Sync>>
{
    let tables = club_tables::list_assigned_to_tournament(pool, tournament_id).await?;
    if tables.is_empty() {
        return Ok(None);
    }

    let mut tx = pool.begin().await?;

    let current =
        table_seat_assignments::list_current_for_tournament(&mut *tx, tournament_id).await?;
    if current.iter().any(|a| a.club_player_id == club_player_id) {
        return Ok(None); // already seated
    }

    let mut occupied: HashMap<Uuid, HashSet<i32>> = HashMap::new();
    for a in &current {
        occupied
            .entry(a.club_table_id)
            .or_default()
            .insert(a.seat_number);
    }
    let mut fills: Vec<TableFill> = tables
        .iter()
        .map(|t| TableFill {
            id: t.id,
            max_seats: t.max_seats,
            occupied: occupied.remove(&t.id).unwrap_or_default(),
        })
        .collect();

    let pick = {
        let mut rng = rand::rng();
        pick_random_seat(&mut fills, &mut rng)
    };
    let Some((club_table_id, seat_number)) = pick else {
        return Ok(None); // no free seat
    };

    let assignment = table_seat_assignments::create(
        &mut *tx,
        CreateSeatAssignment {
            tournament_id,
            club_table_id,
            // The DB link trigger stamps user_id from the roster identity when
            // the player has an app account.
            user_id: None,
            club_player_id: Some(club_player_id),
            seat_number,
            stack_size: None,
            assigned_by: Some(manager_id),
            notes: Some("Auto-seated".to_string()),
        },
    )
    .await?;

    tournament_registrations::update_status_by_club_player(
        &mut *tx,
        tournament_id,
        club_player_id,
        "seated",
    )
    .await?;

    tx.commit().await?;
    Ok(Some(assignment))
}

/// Randomly seat every CHECKED_IN player who has no current seat across the
/// tournament's linked tables, keeping the tables balanced. Intended for the
/// registration-open -> late-registration transition (the "seat draw").
///
/// Players are shuffled, then each is placed on the least-filled table with a
/// free seat, taking a random free seat there; their registration moves to
/// SEATED. Returns the created assignments so the caller can publish events.
/// A no-op (empty result) when there are no linked tables, no checked-in
/// players, or no free seats left.
pub async fn auto_seat_checked_in(
    pool: &sqlx::PgPool,
    tournament_id: Uuid,
    manager_id: Uuid,
) -> Result<AutoSeatResult, Box<dyn std::error::Error + Send + Sync>> {
    let tables = club_tables::list_assigned_to_tournament(pool, tournament_id).await?;
    if tables.is_empty() {
        return Ok(AutoSeatResult {
            assignments: Vec::new(),
        });
    }

    let mut tx = pool.begin().await?;

    // Current seating: which roster players already have a seat, and which seats
    // are taken on each table.
    let current =
        table_seat_assignments::list_current_for_tournament(&mut *tx, tournament_id).await?;
    let seated: HashSet<Uuid> = current.iter().map(|a| a.club_player_id).collect();
    let mut occupied: HashMap<Uuid, HashSet<i32>> = HashMap::new();
    for a in &current {
        occupied
            .entry(a.club_table_id)
            .or_default()
            .insert(a.seat_number);
    }

    // Eligible = checked-in registrations with a roster identity and no seat yet.
    let mut eligible: Vec<_> =
        tournament_registrations::list_by_tournament(&mut *tx, tournament_id)
            .await?
            .into_iter()
            .filter(|r| r.status == "checked_in")
            .filter(|r| !seated.contains(&r.club_player_id))
            .collect();

    let mut fills: Vec<TableFill> = tables
        .iter()
        .map(|t| TableFill {
            id: t.id,
            max_seats: t.max_seats,
            occupied: occupied.remove(&t.id).unwrap_or_default(),
        })
        .collect();

    // Decide the whole draw synchronously so the (non-Send) RNG never crosses an
    // await point. Each player lands on the least-filled table with room, taking
    // a random free seat there.
    let mut plan: Vec<SeatPlan> = Vec::new();
    {
        let mut rng = rand::rng();
        eligible.shuffle(&mut rng);
        for reg in &eligible {
            let Some((club_table_id, seat_number)) = pick_random_seat(&mut fills, &mut rng) else {
                break; // every table is full
            };
            plan.push(SeatPlan {
                club_player_id: reg.club_player_id,
                user_id: reg.user_id,
                club_table_id,
                seat_number,
                stack_size: reg.starting_stack,
            });
        }
    }

    let mut assignments = Vec::new();
    for seat in plan {
        let assignment = table_seat_assignments::create(
            &mut *tx,
            CreateSeatAssignment {
                tournament_id,
                club_table_id: seat.club_table_id,
                user_id: seat.user_id,
                club_player_id: Some(seat.club_player_id),
                seat_number: seat.seat_number,
                stack_size: seat.stack_size,
                assigned_by: Some(manager_id),
                notes: Some("Auto-seated at late registration".to_string()),
            },
        )
        .await?;

        tournament_registrations::update_status_by_club_player(
            &mut *tx,
            tournament_id,
            seat.club_player_id,
            "seated",
        )
        .await?;

        assignments.push(assignment);
    }

    tx.commit().await?;

    Ok(AutoSeatResult { assignments })
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
            sorted_players.sort_by_key(|b| std::cmp::Reverse(b.assigned_at));
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
                        player.club_player_id,
                        Some(params.manager_id),
                    )
                    .await?;

                    let new_assignment = table_seat_assignments::create(
                        &mut *tx,
                        CreateSeatAssignment {
                            tournament_id: params.tournament_id,
                            club_table_id: target_table.id,
                            user_id: player.user_id,
                            club_player_id: Some(player.club_player_id),
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
