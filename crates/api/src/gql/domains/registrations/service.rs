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
    let _updated_registration = tournament_registrations::get_by_tournament_and_user(
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

                // Update registration status to seated
                tournament_registrations::update_status(
                    &mut *tx,
                    params.tournament_id,
                    params.user_id,
                    "seated",
                )
                .await?;

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

    // Re-fetch the registration to reflect any status changes (e.g. seated)
    let final_registration = tournament_registrations::get_by_tournament_and_user(
        &mut *tx,
        params.tournament_id,
        params.user_id,
    )
    .await?
    .ok_or("Failed to get final registration")?;

    // Commit transaction
    tx.commit().await?;

    Ok(CheckInResult {
        updated_registration: final_registration,
        seat_assignment,
        message,
    })
}

/// Parameters for the self-check-in operation (player checks themselves in via QR).
pub struct SelfCheckInParams {
    pub tournament_id: Uuid,
    pub user_id: Uuid,
    pub auto_assign: bool,
    pub assignment_strategy: AssignmentStrategy,
}

/// Result of a self-check-in.
pub struct SelfCheckInResult {
    pub updated_registration: infra::models::TournamentRegistrationRow,
    pub seat_assignment: Option<infra::models::TableSeatAssignmentRow>,
    pub message: String,
    pub was_registered: bool,
}

/// Perform self-check-in: if the player is not registered, register them first,
/// then check them in. Uses JWT auth (no manager required).
pub async fn self_check_in(
    pool: &sqlx::PgPool,
    params: SelfCheckInParams,
) -> Result<SelfCheckInResult, Box<dyn std::error::Error + Send + Sync>> {
    let mut tx = pool.begin().await?;

    // Lock the tournament row
    let tournament = sqlx::query_as::<_, infra::models::TournamentRow>(
        "SELECT id, club_id, name, description, start_time, end_time, buy_in_cents, rake_cents, seat_cap, live_status, early_bird_bonus_chips, late_registration_level, created_at, updated_at FROM tournaments WHERE id = $1 FOR UPDATE",
    )
    .bind(params.tournament_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or("Tournament not found")?;

    // Check tournament status allows check-in
    use infra::repos::tournaments::TournamentLiveStatus;
    match tournament.live_status {
        TournamentLiveStatus::RegistrationOpen
        | TournamentLiveStatus::LateRegistration
        | TournamentLiveStatus::InProgress => { /* allowed */ }
        _ => {
            return Err(format!(
                "Tournament is not accepting check-ins (status: {:?})",
                tournament.live_status
            )
            .into());
        }
    }

    // Check if user is already registered
    let existing = tournament_registrations::get_by_tournament_and_user(
        &mut *tx,
        params.tournament_id,
        params.user_id,
    )
    .await?;

    let was_registered;

    match &existing {
        Some(reg) => {
            match reg.status.as_str() {
                "registered" => {
                    // Already registered, proceed to check-in
                    was_registered = false;
                }
                "checked_in" | "seated" => {
                    // Already checked in or seated
                    tx.commit().await?;
                    return Ok(SelfCheckInResult {
                        updated_registration: reg.clone(),
                        seat_assignment: None,
                        message: format!("You are already checked in for {}", tournament.name),
                        was_registered: false,
                    });
                }
                "waitlisted" => {
                    tx.commit().await?;
                    return Err(
                        "You are on the waitlist. Please wait for a spot to open up.".into(),
                    );
                }
                "cancelled" | "no_show" => {
                    tx.commit().await?;
                    return Err(format!(
                        "Your registration was {}. Please contact the tournament manager.",
                        reg.status
                    )
                    .into());
                }
                other => {
                    tx.commit().await?;
                    return Err(format!("Cannot check in from status: {}", other).into());
                }
            }
        }
        None => {
            // Not registered - only allow new registrations during REGISTRATION_OPEN or LATE_REGISTRATION
            match tournament.live_status {
                TournamentLiveStatus::RegistrationOpen | TournamentLiveStatus::LateRegistration => { /* allowed */
                }
                _ => {
                    tx.commit().await?;
                    return Err("Registration is not open for this tournament".into());
                }
            }

            // Check seat capacity
            let is_waitlisted = if let Some(seat_cap) = tournament.seat_cap {
                let confirmed_count = tournament_registrations::count_confirmed_by_tournament(
                    &mut *tx,
                    params.tournament_id,
                )
                .await?;
                confirmed_count >= seat_cap as i64
            } else {
                false
            };

            if is_waitlisted {
                // Can't auto-check-in if waitlisted
                let create_data = tournament_registrations::CreateTournamentRegistration {
                    tournament_id: params.tournament_id,
                    user_id: params.user_id,
                    notes: Some("Self-registered via QR scan".to_string()),
                    status: Some("waitlisted".to_string()),
                };
                let row = tournament_registrations::create(&mut *tx, create_data).await?;
                tx.commit().await?;
                return Ok(SelfCheckInResult {
                    updated_registration: row,
                    seat_assignment: None,
                    message: format!(
                        "Tournament is full. You have been added to the waitlist for {}",
                        tournament.name
                    ),
                    was_registered: true,
                });
            }

            // Register as confirmed
            let create_data = tournament_registrations::CreateTournamentRegistration {
                tournament_id: params.tournament_id,
                user_id: params.user_id,
                notes: Some("Self-registered via QR scan".to_string()),
                status: None, // defaults to 'registered'
            };
            tournament_registrations::create(&mut *tx, create_data).await?;
            was_registered = true;
        }
    }

    // Update status to checked_in
    tournament_registrations::update_status(
        &mut *tx,
        params.tournament_id,
        params.user_id,
        "checked_in",
    )
    .await?;

    // Get updated registration
    let _updated_registration = tournament_registrations::get_by_tournament_and_user(
        &mut *tx,
        params.tournament_id,
        params.user_id,
    )
    .await?
    .ok_or("Failed to get updated registration")?;

    // Auto-assign to table
    let mut seat_assignment: Option<infra::models::TableSeatAssignmentRow> = None;
    let mut message = if was_registered {
        format!("Registered and checked in for {}", tournament.name)
    } else {
        format!("Checked in for {}", tournament.name)
    };

    if params.auto_assign && params.assignment_strategy != AssignmentStrategy::Manual {
        let tables = club_tables::list_assigned_to_tournament(pool, params.tournament_id).await?;

        if !tables.is_empty() {
            let current_assignments =
                table_seat_assignments::list_current_for_tournament(&mut *tx, params.tournament_id)
                    .await?;

            let mut table_counts: std::collections::HashMap<Uuid, usize> =
                std::collections::HashMap::new();
            for assignment in &current_assignments {
                *table_counts.entry(assignment.club_table_id).or_insert(0) += 1;
            }

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
                    assigned_by: None, // Self check-in, no manager
                    notes: Some(format!(
                        "Auto-assigned on self check-in using {:?} strategy",
                        params.assignment_strategy
                    )),
                };

                let assignment_row = table_seat_assignments::create(&mut *tx, create_data).await?;

                // Update registration status to seated
                tournament_registrations::update_status(
                    &mut *tx,
                    params.tournament_id,
                    params.user_id,
                    "seated",
                )
                .await?;

                message = if was_registered {
                    format!(
                        "Registered, checked in and assigned to Table {}, Seat {}",
                        target_table.table_number, seat_num
                    )
                } else {
                    format!(
                        "Checked in and assigned to Table {}, Seat {}",
                        target_table.table_number, seat_num
                    )
                };

                seat_assignment = Some(assignment_row);
            }
        }
    }

    // Re-fetch the registration to reflect any status changes (e.g. seated)
    let final_registration = tournament_registrations::get_by_tournament_and_user(
        &mut *tx,
        params.tournament_id,
        params.user_id,
    )
    .await?
    .ok_or("Failed to get final registration")?;

    tx.commit().await?;

    Ok(SelfCheckInResult {
        updated_registration: final_registration,
        seat_assignment,
        message,
        was_registered,
    })
}

/// Result of a waitlist promotion.
pub struct PromotionResult {
    pub promoted_registration: infra::models::TournamentRegistrationRow,
}

/// Promote the next waitlisted player if there is capacity.
/// Returns the promoted player's registration, or None if no promotion needed/possible.
pub async fn promote_next_waitlisted(
    pool: &sqlx::PgPool,
    tournament_id: Uuid,
) -> Result<Option<PromotionResult>, Box<dyn std::error::Error + Send + Sync>> {
    let mut tx = pool.begin().await?;

    // Lock the tournament row
    let tournament = sqlx::query_as::<_, infra::models::TournamentRow>(
        "SELECT id, club_id, name, description, start_time, end_time, buy_in_cents, rake_cents, seat_cap, live_status, early_bird_bonus_chips, late_registration_level, created_at, updated_at FROM tournaments WHERE id = $1 FOR UPDATE",
    )
    .bind(tournament_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or("Tournament not found")?;

    let seat_cap = match tournament.seat_cap {
        Some(cap) => cap as i64,
        None => {
            // No seat cap, no promotion needed
            tx.commit().await?;
            return Ok(None);
        }
    };

    let confirmed_count =
        tournament_registrations::count_confirmed_by_tournament(&mut *tx, tournament_id).await?;

    if confirmed_count >= seat_cap {
        // Still full, no promotion
        tx.commit().await?;
        return Ok(None);
    }

    // Get the next waitlisted player
    let next_waitlisted =
        tournament_registrations::get_next_waitlisted(&mut *tx, tournament_id).await?;

    match next_waitlisted {
        Some(waitlisted) => {
            // Promote to registered
            tournament_registrations::update_status(
                &mut *tx,
                tournament_id,
                waitlisted.user_id,
                "registered",
            )
            .await?;

            // Get updated registration
            let updated = tournament_registrations::get_by_tournament_and_user(
                &mut *tx,
                tournament_id,
                waitlisted.user_id,
            )
            .await?
            .ok_or("Failed to get updated registration")?;

            tx.commit().await?;

            Ok(Some(PromotionResult {
                promoted_registration: updated,
            }))
        }
        None => {
            tx.commit().await?;
            Ok(None)
        }
    }
}
