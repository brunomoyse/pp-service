use std::collections::{HashMap, HashSet};

use rand::seq::SliceRandom;
use uuid::Uuid;

use infra::models::{ClubTableRow, TableSeatAssignmentRow};
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
    pub moves: Vec<TableSeatAssignmentRow>,
}

/// Result of an auto-seat operation.
pub struct AutoSeatResult {
    pub assignments: Vec<TableSeatAssignmentRow>,
}

/// Balance assessment for a tournament's tables, surfaced to the manager UI so
/// the director knows when to rebalance/consolidate (like a real tournament).
pub struct BalanceStatus {
    /// Player spread between active tables exceeds the TDA threshold.
    pub needs_rebalance: bool,
    /// The field now fits on fewer tables than are currently in use.
    pub needs_consolidation: bool,
    /// Spread is 3+ players — play should halt on the short table.
    pub critical: bool,
    /// Minimal number of tables the current field should occupy.
    pub suggested_table_count: i32,
}

/// Working state for one table while filling free seats.
struct TableFill {
    id: Uuid,
    table_number: i32,
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

/// Minimal number of tables whose summed capacity covers `total` players.
///
/// This is the casino rule that keeps tables *playable*: open the fewest tables
/// the field requires (`ceil(total / capacity)` for uniform tables) instead of
/// spreading players thin. Always at least 1.
fn min_tables_needed(total: i32, caps: &[i32]) -> usize {
    if total <= 0 {
        return 1;
    }
    let mut caps_desc: Vec<i32> = caps.iter().copied().filter(|c| *c > 0).collect();
    caps_desc.sort_unstable_by(|a, b| b.cmp(a));
    let mut sum: i64 = 0;
    let mut n: usize = 0;
    for c in caps_desc {
        if sum >= total as i64 {
            break;
        }
        sum += c as i64;
        n += 1;
    }
    n.max(1)
}

/// Build per-table fill state from the tournament's linked tables and the
/// current seat assignments.
fn build_fills(tables: &[ClubTableRow], current: &[TableSeatAssignmentRow]) -> Vec<TableFill> {
    let mut occupied: HashMap<Uuid, HashSet<i32>> = HashMap::new();
    for a in current {
        occupied
            .entry(a.club_table_id)
            .or_default()
            .insert(a.seat_number);
    }
    tables
        .iter()
        .map(|t| TableFill {
            id: t.id,
            table_number: t.table_number,
            max_seats: t.max_seats,
            occupied: occupied.remove(&t.id).unwrap_or_default(),
        })
        .collect()
}

/// Fill-then-balance seat picker.
///
/// `total_after` is the seated count *including* the player being placed. We
/// only ever spread across `min_tables_needed(total_after)` tables — preferring
/// tables that already have players, then the lowest table numbers — and within
/// that active set we take the least-filled table (balancing it) and a random
/// free seat. Extra linked tables stay empty until the field actually needs
/// them, so two players land at the *same* table instead of one each.
///
/// Synchronous so the RNG never crosses an await point.
fn pick_fill_then_balance(
    fills: &mut [TableFill],
    total_after: i32,
    rng: &mut impl rand::RngExt,
) -> Option<(Uuid, i32)> {
    if fills.is_empty() {
        return None;
    }

    let caps: Vec<i32> = fills.iter().map(|f| f.max_seats).collect();
    let target = min_tables_needed(total_after, &caps);

    // Active set = `target` tables, occupied ones first, then by table number.
    let mut order: Vec<usize> = (0..fills.len()).collect();
    order.sort_by_key(|&i| (fills[i].occupied.is_empty(), fills[i].table_number));
    let active: HashSet<usize> = order.into_iter().take(target).collect();

    // Least-filled active table with a free seat. Fall back to any table with a
    // free seat so seating never fails while capacity remains (e.g. uneven caps).
    let idx = (0..fills.len())
        .filter(|&i| active.contains(&i) && (fills[i].occupied.len() as i32) < fills[i].max_seats)
        .min_by_key(|&i| (fills[i].occupied.len(), fills[i].table_number))
        .or_else(|| {
            (0..fills.len())
                .filter(|&i| (fills[i].occupied.len() as i32) < fills[i].max_seats)
                .min_by_key(|&i| (fills[i].occupied.len(), fills[i].table_number))
        })?;

    let table = &mut fills[idx];
    let free: Vec<i32> = (1..=table.max_seats)
        .filter(|n| !table.occupied.contains(n))
        .collect();
    let seat_number = free[rng.random_range(0..free.len())];
    table.occupied.insert(seat_number);
    Some((table.id, seat_number))
}

/// Decide a single fill-then-balance seat for an externally-managed
/// transaction (used by the check-in path). `total_after` is the seated count
/// including this player. Synchronous; the RNG never crosses an await point.
pub fn decide_seat_fill_then_balance(
    tables: &[ClubTableRow],
    current: &[TableSeatAssignmentRow],
    total_after: i32,
) -> Option<(Uuid, i32)> {
    let mut fills = build_fills(tables, current);
    let mut rng = rand::rng();
    pick_fill_then_balance(&mut fills, total_after, &mut rng)
}

/// Seat one checked-in player using fill-then-balance across the linked tables,
/// moving their registration to SEATED. Returns `None` when there are no
/// tables, no free seat, or the player already holds a seat. Pure DB work; the
/// caller publishes the seating event and logs.
pub async fn auto_seat_one(
    pool: &sqlx::PgPool,
    tournament_id: Uuid,
    club_player_id: Uuid,
    manager_id: Uuid,
) -> Result<Option<TableSeatAssignmentRow>, Box<dyn std::error::Error + Send + Sync>> {
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

    let mut fills = build_fills(&tables, &current);
    // Including the player we're about to place.
    let total_after = current.len() as i32 + 1;

    let pick = {
        let mut rng = rand::rng();
        pick_fill_then_balance(&mut fills, total_after, &mut rng)
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

/// Seat every CHECKED_IN player who has no current seat using fill-then-balance
/// across the tournament's linked tables. Intended for the
/// registration-open -> late-registration transition (the "seat draw").
///
/// This is a redraw: the whole eligible field is sized first, so the players
/// are split evenly across only the minimal number of tables the field needs
/// (e.g. 11 players / 9-max -> 6 + 5 on two tables, a third linked table stays
/// empty). Players are shuffled, each placed on the least-filled active table
/// with a random free seat; their registration moves to SEATED. A no-op when
/// there are no linked tables, no checked-in players, or no free seats left.
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

    // Eligible = checked-in registrations with a roster identity and no seat yet.
    let mut eligible: Vec<_> =
        tournament_registrations::list_by_tournament(&mut *tx, tournament_id)
            .await?
            .into_iter()
            .filter(|r| r.status == "checked_in")
            .filter(|r| !seated.contains(&r.club_player_id))
            .collect();

    let mut fills = build_fills(&tables, &current);
    // Size the whole draw up-front so the active-table set (and the even split
    // across it) is fixed for every placement in this draw.
    let total_after = current.len() as i32 + eligible.len() as i32;

    // Decide the whole draw synchronously so the (non-Send) RNG never crosses an
    // await point.
    let mut plan: Vec<SeatPlan> = Vec::new();
    {
        let mut rng = rand::rng();
        eligible.shuffle(&mut rng);
        for reg in &eligible {
            let Some((club_table_id, seat_number)) =
                pick_fill_then_balance(&mut fills, total_after, &mut rng)
            else {
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

/// Assess table balance using TDA-style thresholds, for surfacing a warning to
/// the manager. `table_counts` is the player count per table (any linked table,
/// including empty ones); `caps` the capacities of all linked tables.
///
/// Thresholds (Tournament Directors Association):
/// - Active tables kept within 1 player (<= 6 tables) or 2 players (> 6 tables).
/// - A 3+ player spread is critical (play halts on the short table).
/// - Consolidate when more tables are in use than the field needs.
pub fn assess_balance(table_counts: &HashMap<Uuid, usize>, caps: &[i32]) -> BalanceStatus {
    let counts: Vec<usize> = table_counts.values().copied().filter(|c| *c > 0).collect();
    let active = counts.len();
    if active == 0 {
        return BalanceStatus {
            needs_rebalance: false,
            needs_consolidation: false,
            critical: false,
            suggested_table_count: 0,
        };
    }

    let min = *counts.iter().min().unwrap();
    let max = *counts.iter().max().unwrap();
    let spread = max - min;
    let max_diff = if active <= 6 { 1 } else { 2 };

    let total: usize = counts.iter().sum();
    let target = min_tables_needed(total as i32, caps);

    BalanceStatus {
        needs_rebalance: active > 1 && spread > max_diff,
        needs_consolidation: active > target,
        critical: active > 1 && spread >= 3,
        suggested_table_count: target as i32,
    }
}

/// Perform the table balancing + consolidation workflow inside a transaction.
///
/// First the field is consolidated onto the minimal number of tables it needs
/// (the fullest tables are kept; shorter ones are emptied, their players moved
/// to open seats on the kept tables). Then the kept tables are evened out so no
/// two differ by more than one player. Emptied tables stay linked but free, so
/// the manager can break them with the existing button (non-destructive).
///
/// The caller (resolver) is responsible for auth, ID parsing, GraphQL
/// conversion, and publishing subscription events.
pub async fn balance_tables(
    pool: &sqlx::PgPool,
    params: BalanceParams,
) -> Result<BalanceResult, Box<dyn std::error::Error + Send + Sync>> {
    let tables = club_tables::list_assigned_to_tournament(pool, params.tournament_id).await?;
    if tables.is_empty() {
        return Ok(BalanceResult { moves: Vec::new() });
    }

    let mut tx = pool.begin().await?;

    let assignments =
        table_seat_assignments::list_current_for_tournament(&mut *tx, params.tournament_id).await?;

    // Group players by table.
    let mut table_players: HashMap<Uuid, Vec<TableSeatAssignmentRow>> = HashMap::new();
    for assignment in assignments {
        table_players
            .entry(assignment.club_table_id)
            .or_default()
            .push(assignment);
    }
    let table_counts: HashMap<Uuid, usize> = table_players
        .iter()
        .map(|(id, players)| (*id, players.len()))
        .collect();

    let caps: Vec<i32> = tables.iter().map(|t| t.max_seats).collect();
    let status = assess_balance(&table_counts, &caps);

    let total: usize = table_counts.values().sum();
    if total == 0 {
        return Ok(BalanceResult { moves: Vec::new() });
    }

    // Target number of active tables. An explicit per-table target from the
    // manager overrides the casino default.
    let target_tables = match params.target_players_per_table {
        Some(n) if n > 0 => ((total as f64) / (n as f64)).ceil() as usize,
        _ => status.suggested_table_count.max(1) as usize,
    }
    .clamp(1, tables.len());

    // Nothing to do: already balanced and on the right number of tables (unless
    // the manager forced a specific per-table target).
    if params.target_players_per_table.is_none()
        && !status.needs_rebalance
        && !status.needs_consolidation
    {
        return Ok(BalanceResult { moves: Vec::new() });
    }

    // Keep the fullest tables (fewest moves), then by table number.
    let mut ordered: Vec<&ClubTableRow> = tables.iter().collect();
    ordered.sort_by_key(|t| {
        (
            std::cmp::Reverse(table_counts.get(&t.id).copied().unwrap_or(0)),
            t.table_number,
        )
    });
    let keep: Vec<&ClubTableRow> = ordered.into_iter().take(target_tables).collect();
    let keep_ids: HashSet<Uuid> = keep.iter().map(|t| t.id).collect();

    // Even split across the kept tables; the fullest tables absorb the
    // remainder so the fewest players move.
    let base = total / target_tables;
    let rem = total % target_tables;
    let mut desired: HashMap<Uuid, usize> = HashMap::new();
    for (i, t) in keep.iter().enumerate() {
        desired.insert(t.id, base + if i < rem { 1 } else { 0 });
    }

    // Track occupied seats per kept table in-memory.
    let mut occupied: HashMap<Uuid, HashSet<i32>> = HashMap::new();
    for t in &keep {
        let set: HashSet<i32> = table_players
            .get(&t.id)
            .map(|v| v.iter().map(|a| a.seat_number).collect())
            .unwrap_or_default();
        occupied.insert(t.id, set);
    }

    // Movable pool: everyone on a non-kept table, plus the excess on kept tables
    // (most-recently-seated leave first so established players stay put).
    let mut movable: Vec<TableSeatAssignmentRow> = Vec::new();
    for t in &tables {
        let empty = Vec::new();
        let players = table_players.get(&t.id).unwrap_or(&empty);
        if keep_ids.contains(&t.id) {
            let want = *desired.get(&t.id).unwrap_or(&0);
            if players.len() > want {
                let mut sorted = players.clone();
                sorted.sort_by_key(|a| std::cmp::Reverse(a.assigned_at));
                for p in sorted.into_iter().take(players.len() - want) {
                    if let Some(set) = occupied.get_mut(&t.id) {
                        set.remove(&p.seat_number);
                    }
                    movable.push(p);
                }
            }
        } else {
            movable.extend(players.clone());
        }
    }

    // Place each movable player onto a kept table that is under its desired
    // count; fall back to any kept table with a free seat.
    let mut moves = Vec::new();
    for player in movable {
        let target = keep
            .iter()
            .find(|t| {
                let cur = occupied.get(&t.id).map(|s| s.len()).unwrap_or(0);
                cur < *desired.get(&t.id).unwrap_or(&0) && cur < t.max_seats as usize
            })
            .or_else(|| {
                keep.iter().find(|t| {
                    occupied.get(&t.id).map(|s| s.len()).unwrap_or(0) < t.max_seats as usize
                })
            });
        let Some(target_table) = target else {
            continue;
        };

        let set = occupied.entry(target_table.id).or_default();
        let Some(seat_num) = (1..=target_table.max_seats).find(|s| !set.contains(s)) else {
            continue;
        };

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

        occupied
            .entry(target_table.id)
            .or_default()
            .insert(seat_num);
        moves.push(new_assignment);
    }

    tx.commit().await?;

    Ok(BalanceResult { moves })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fills(caps: &[i32]) -> Vec<TableFill> {
        caps.iter()
            .enumerate()
            .map(|(i, &c)| TableFill {
                id: Uuid::from_u128(i as u128 + 1),
                table_number: i as i32 + 1,
                max_seats: c,
                occupied: HashSet::new(),
            })
            .collect()
    }

    /// Seat `n` players as a single draw (target sized up-front), returning the
    /// resulting per-table counts sorted descending.
    fn run_draw(caps: &[i32], n: usize) -> Vec<usize> {
        let mut f = fills(caps);
        let mut rng = rand::rng();
        for _ in 0..n {
            pick_fill_then_balance(&mut f, n as i32, &mut rng).expect("a seat should be found");
        }
        let mut counts: Vec<usize> = f.iter().map(|t| t.occupied.len()).collect();
        counts.sort_unstable_by(|a, b| b.cmp(a));
        counts
    }

    /// Seat `n` players one at a time (target recomputed each call).
    fn run_incremental(caps: &[i32], n: usize) -> Vec<usize> {
        let mut f = fills(caps);
        let mut rng = rand::rng();
        for i in 0..n {
            pick_fill_then_balance(&mut f, (i + 1) as i32, &mut rng)
                .expect("a seat should be found");
        }
        let mut counts: Vec<usize> = f.iter().map(|t| t.occupied.len()).collect();
        counts.sort_unstable_by(|a, b| b.cmp(a));
        counts
    }

    fn counts(values: &[usize]) -> HashMap<Uuid, usize> {
        values
            .iter()
            .enumerate()
            .map(|(i, &c)| (Uuid::from_u128(i as u128 + 1), c))
            .collect()
    }

    #[test]
    fn min_tables_needed_uniform() {
        assert_eq!(min_tables_needed(0, &[9, 9, 9]), 1);
        assert_eq!(min_tables_needed(2, &[9, 9, 9]), 1);
        assert_eq!(min_tables_needed(9, &[9, 9, 9]), 1);
        assert_eq!(min_tables_needed(10, &[9, 9, 9]), 2);
        assert_eq!(min_tables_needed(11, &[9, 9, 9]), 2);
        assert_eq!(min_tables_needed(18, &[9, 9, 9]), 2);
        assert_eq!(min_tables_needed(19, &[9, 9, 9]), 3);
    }

    #[test]
    fn min_tables_needed_uneven_caps() {
        // Sorted desc [9, 6]: 9 covers <10, add 6 -> two tables.
        assert_eq!(min_tables_needed(10, &[6, 9]), 2);
        // 6 covers 6 with the bigger table alone.
        assert_eq!(min_tables_needed(6, &[6, 9]), 1);
    }

    #[test]
    fn draw_two_players_share_one_table() {
        // The reported bug: two players must NOT spread across two tables.
        assert_eq!(run_draw(&[9, 9], 2), vec![2, 0]);
        assert_eq!(run_draw(&[9, 9, 9], 2), vec![2, 0, 0]);
    }

    #[test]
    fn draw_splits_evenly_over_minimal_tables() {
        // 11 / 9-max -> 6 + 5 on two tables, third stays empty.
        assert_eq!(run_draw(&[9, 9, 9], 11), vec![6, 5, 0]);
        // Two full tables, third empty.
        assert_eq!(run_draw(&[9, 9, 9], 18), vec![9, 9, 0]);
        // Exactly one full table.
        assert_eq!(run_draw(&[9, 9, 9], 9), vec![9, 0, 0]);
        // 10 -> 5 + 5.
        assert_eq!(run_draw(&[9, 9, 9], 10), vec![5, 5, 0]);
    }

    #[test]
    fn incremental_fills_before_opening_new_table() {
        // Two players one-by-one still share a table.
        assert_eq!(run_incremental(&[9, 9], 2), vec![2, 0]);
        // A new table only opens once the first is full.
        assert_eq!(run_incremental(&[9, 9, 9], 9), vec![9, 0, 0]);
        assert_eq!(run_incremental(&[9, 9, 9], 10), vec![9, 1, 0]);
    }

    #[test]
    fn assess_balance_thresholds() {
        let caps = [9, 9, 9];

        // 6/5 on two tables: balanced, right number of tables.
        let s = assess_balance(&counts(&[6, 5]), &caps);
        assert!(!s.needs_rebalance && !s.needs_consolidation && !s.critical);
        assert_eq!(s.suggested_table_count, 2);

        // 9/2: spread 7 -> rebalance + critical.
        let s = assess_balance(&counts(&[9, 2]), &[9, 9]);
        assert!(s.needs_rebalance && s.critical);
        assert!(!s.needs_consolidation);

        // 4/4/3 across three tables: within 1 but the field fits on two.
        let s = assess_balance(&counts(&[4, 4, 3]), &caps);
        assert!(!s.needs_rebalance);
        assert!(s.needs_consolidation);
        assert_eq!(s.suggested_table_count, 2);

        // Single active table is never "unbalanced".
        let s = assess_balance(&counts(&[5]), &caps);
        assert!(!s.needs_rebalance && !s.critical && !s.needs_consolidation);
    }

    #[test]
    fn assess_balance_wider_tolerance_above_six_tables() {
        // 7 active tables, spread of 2 is allowed (within-2 above 6 tables).
        let caps = [9; 7];
        let s = assess_balance(&counts(&[7, 5, 5, 5, 5, 5, 5]), &caps);
        assert!(!s.needs_rebalance);
        // Spread of 3 trips both the threshold and the critical flag.
        let s = assess_balance(&counts(&[8, 5, 5, 5, 5, 5, 5]), &caps);
        assert!(s.needs_rebalance && s.critical);
    }
}
