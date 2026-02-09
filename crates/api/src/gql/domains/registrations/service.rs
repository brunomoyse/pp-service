use rand::RngExt;
use uuid::Uuid;

use infra::repos::{
    club_tables, table_seat_assignments, table_seat_assignments::CreateSeatAssignment,
    tournament_entries, tournament_registrations, tournaments,
};

use crate::gql::types::AssignmentStrategy;

/// Parameters for the check-in operation (parsed by the resolver).
pub struct CheckInParams {
    pub tournament_id: Uuid,
    pub user_id: Uuid,
    pub manager_id: Uuid,
    pub auto_assign: bool,
    pub assignment_strategy: AssignmentStrategy,
    pub grant_early_bird_bonus: bool,
}

/// Result of a successful check-in.
pub struct CheckInResult {
    pub updated_registration: infra::models::TournamentRegistrationRow,
    pub seat_assignment: Option<infra::models::TableSeatAssignmentRow>,
    pub message: String,
}

/// Perform the check-in workflow inside a transaction.
///
/// The caller (resolver) is responsible for:
/// - Authentication / authorization
/// - Parsing IDs from GraphQL input
/// - Converting the result to GraphQL types
/// - Publishing subscription events
pub async fn check_in_player(
    pool: &sqlx::PgPool,
    params: CheckInParams,
) -> Result<CheckInResult, Box<dyn std::error::Error + Send + Sync>> {
    // Validate current registration status
    let registration = tournament_registrations::get_by_tournament_and_user(
        pool,
        params.tournament_id,
        params.user_id,
    )
    .await?
    .ok_or("Player not registered for this tournament")?;

    if registration.status != "registered" {
        return Err(format!(
            "Player cannot be checked in from status: {}",
            registration.status
        )
        .into());
    }

    // Begin transaction
    let mut tx = pool.begin().await?;

    // Update status to checked_in
    tournament_registrations::update_status(
        &mut *tx,
        params.tournament_id,
        params.user_id,
        "checked_in",
    )
    .await?;

    // Get updated registration
    let updated_registration = tournament_registrations::get_by_tournament_and_user(
        &mut *tx,
        params.tournament_id,
        params.user_id,
    )
    .await?
    .ok_or("Failed to get updated registration")?;

    // Apply early bird bonus if requested
    if params.grant_early_bird_bonus {
        let tournament = tournaments::get_by_id(pool, params.tournament_id)
            .await?
            .ok_or("Tournament not found")?;

        if let Some(bonus_chips) = tournament.early_bird_bonus_chips {
            tournament_entries::apply_early_bird_bonus(
                &mut *tx,
                params.tournament_id,
                params.user_id,
                bonus_chips,
            )
            .await?;
        }
    }

    // Auto-assign to table
    let mut seat_assignment: Option<infra::models::TableSeatAssignmentRow> = None;
    let mut message = String::from("Player checked in successfully");

    if params.auto_assign && params.assignment_strategy != AssignmentStrategy::Manual {
        let tables = club_tables::list_assigned_to_tournament(pool, params.tournament_id).await?;

        if !tables.is_empty() {
            let current_assignments =
                table_seat_assignments::list_current_for_tournament(&mut *tx, params.tournament_id)
                    .await?;

            // Count players per table
            let mut table_counts: std::collections::HashMap<Uuid, usize> =
                std::collections::HashMap::new();
            for assignment in &current_assignments {
                *table_counts.entry(assignment.club_table_id).or_insert(0) += 1;
            }

            // Find best table based on strategy
            let target_table = match params.assignment_strategy {
                AssignmentStrategy::Balanced => tables
                    .iter()
                    .min_by_key(|table| table_counts.get(&table.id).unwrap_or(&0))
                    .ok_or("No tables available")?,
                AssignmentStrategy::Random => {
                    use rand::seq::IndexedRandom;
                    tables
                        .choose(&mut rand::rng())
                        .ok_or("No tables available")?
                }
                AssignmentStrategy::Sequential => tables
                    .iter()
                    .find(|table| {
                        let count = table_counts.get(&table.id).unwrap_or(&0);
                        *count < table.max_seats as usize
                    })
                    .ok_or("All tables are full")?,
                _ => unreachable!(),
            };

            // Find available seats
            let occupied_seats: std::collections::HashSet<i32> =
                table_seat_assignments::get_occupied_seats(&mut *tx, target_table.id)
                    .await?
                    .into_iter()
                    .collect();
            let available_seats: Vec<i32> = (1..=target_table.max_seats)
                .filter(|seat| !occupied_seats.contains(seat))
                .collect();

            if !available_seats.is_empty() {
                let random_index = rand::rng().random_range(0..available_seats.len());
                let seat_num = available_seats[random_index];

                let create_data = CreateSeatAssignment {
                    tournament_id: params.tournament_id,
                    club_table_id: target_table.id,
                    user_id: params.user_id,
                    seat_number: seat_num,
                    stack_size: None,
                    assigned_by: Some(params.manager_id),
                    notes: Some(format!(
                        "Auto-assigned on check-in using {:?} strategy",
                        params.assignment_strategy
                    )),
                };

                let assignment_row = table_seat_assignments::create(&mut *tx, create_data).await?;

                message = format!(
                    "Player checked in and assigned to Table {}, Seat {}",
                    target_table.table_number, seat_num
                );

                seat_assignment = Some(assignment_row);
            } else {
                message =
                    "Player checked in but no seats available for auto-assignment".to_string();
            }
        } else {
            message = "Player checked in but no tables assigned to tournament yet".to_string();
        }
    }

    // Commit transaction
    tx.commit().await?;

    Ok(CheckInResult {
        updated_registration,
        seat_assignment,
        message,
    })
}
