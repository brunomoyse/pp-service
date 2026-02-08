use async_graphql::{dataloader::DataLoader, Context, Object, Result, ID};
use chrono::Utc;
use rand::Rng;
use std::collections::HashMap;
use uuid::Uuid;

use crate::gql::error::ResultExt;
use crate::gql::loaders::UserLoader;
use crate::gql::subscriptions::{
    publish_registration_event, publish_seating_event, publish_user_notification,
};
use crate::gql::types::{
    AssignmentStrategy, CheckInPlayerInput, CheckInResponse, NotificationType,
    PlayerRegistrationEvent, RegisterForTournamentInput, RegistrationEventType, RegistrationStatus,
    Role, SeatAssignment, SeatingChangeEvent, SeatingEventType, TournamentPlayer,
    TournamentRegistration, User, UserNotification, TITLE_REGISTRATION_CONFIRMED,
};
use crate::state::AppState;
use infra::repos::{
    club_tables, table_seat_assignments, table_seat_assignments::CreateSeatAssignment,
    tournament_entries, tournament_registrations,
    tournament_registrations::CreateTournamentRegistration, tournaments, users,
};

async fn get_club_id_for_tournament(db: &infra::db::Db, tournament_id: Uuid) -> Result<Uuid> {
    let tournament = tournaments::get_by_id(db, tournament_id)
        .await?
        .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;
    Ok(tournament.club_id)
}

#[derive(Default)]
pub struct RegistrationQuery;

#[Object]
impl RegistrationQuery {
    async fn tournament_players(
        &self,
        ctx: &Context<'_>,
        tournament_id: Uuid,
    ) -> Result<Vec<TournamentPlayer>> {
        use crate::auth::Claims;

        let _claims = ctx
            .data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;

        let state = ctx.data::<AppState>()?;

        // Fetch all registrations for the tournament
        let registrations =
            tournament_registrations::list_by_tournament(&state.db, tournament_id).await?;

        // Collect all user IDs and batch load them using DataLoader
        let user_ids: Vec<Uuid> = registrations.iter().map(|r| r.user_id).collect();
        let user_loader = ctx.data::<DataLoader<UserLoader>>()?;
        let users: HashMap<Uuid, infra::models::UserRow> = user_loader
            .load_many(user_ids)
            .await
            .gql_err("Data loading failed")?;

        // Build players by looking up users from the HashMap
        let mut players = Vec::new();
        for registration in registrations {
            if let Some(user_row) = users.get(&registration.user_id) {
                let tournament_registration = TournamentRegistration {
                    id: registration.id.into(),
                    tournament_id: registration.tournament_id.into(),
                    user_id: registration.user_id.into(),
                    registration_time: registration.registration_time,
                    status: registration.status.into(),
                    notes: registration.notes.clone(),
                };

                let user = User {
                    id: user_row.id.into(),
                    email: user_row.email.clone(),
                    username: user_row.username.clone(),
                    first_name: user_row.first_name.clone(),
                    last_name: user_row.last_name.clone(),
                    phone: user_row.phone.clone(),
                    is_active: user_row.is_active,
                    role: Role::from(user_row.role.clone()),
                };

                players.push(TournamentPlayer {
                    registration: tournament_registration,
                    user,
                });
            }
        }

        Ok(players)
    }

    async fn my_tournament_registrations(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<TournamentRegistration>> {
        use crate::auth::Claims;

        // Get authenticated user from JWT token
        let claims = ctx
            .data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;

        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let state = ctx.data::<AppState>()?;

        let registrations = tournament_registrations::list_user_current(&state.db, user_id).await?;

        Ok(registrations
            .into_iter()
            .map(|r| TournamentRegistration {
                id: r.id.into(),
                tournament_id: r.tournament_id.into(),
                user_id: r.user_id.into(),
                registration_time: r.registration_time,
                status: r.status.into(),
                notes: r.notes,
            })
            .collect())
    }
}

#[derive(Default)]
pub struct RegistrationMutation;

#[Object]
impl RegistrationMutation {
    /// Register a user for a tournament.
    async fn register_for_tournament(
        &self,
        ctx: &Context<'_>,
        input: RegisterForTournamentInput,
    ) -> Result<TournamentRegistration> {
        use crate::auth::permissions::require_admin_if;

        let state = ctx.data::<AppState>()?;

        // Check permissions: require admin role if registering another user
        let is_admin_registration = input.user_id.is_some();
        let authenticated_user = require_admin_if(ctx, is_admin_registration, "user_id")
            .await?
            .ok_or_else(|| {
                async_graphql::Error::new("You must be logged in to perform this action")
            })?;

        // Determine which user to register
        let user_id = match input.user_id {
            Some(target_user_id) => {
                // Admin is registering another user
                Uuid::parse_str(target_user_id.as_str()).gql_err("Invalid target user ID")?
            }
            None => {
                // User registering themselves
                Uuid::parse_str(authenticated_user.id.as_str()).gql_err("Invalid user ID")?
            }
        };

        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let create_data = CreateTournamentRegistration {
            tournament_id,
            user_id,
            notes: input.notes.clone(),
        };

        let row = tournament_registrations::create(&state.db, create_data).await?;

        let tournament_registration = TournamentRegistration {
            id: row.id.into(),
            tournament_id: row.tournament_id.into(),
            user_id: row.user_id.into(),
            registration_time: row.registration_time,
            status: row.status.clone().into(),
            notes: row.notes.clone(),
        };

        // Emit subscription event
        if let Some(user_row) = users::get_by_id(&state.db, user_id).await? {
            let user = User {
                id: user_row.id.into(),
                email: user_row.email,
                username: user_row.username,
                first_name: user_row.first_name,
                last_name: user_row.last_name,
                phone: user_row.phone,
                is_active: user_row.is_active,
                role: Role::from(user_row.role),
            };

            let player = TournamentPlayer {
                registration: tournament_registration.clone(),
                user,
            };

            let event = PlayerRegistrationEvent {
                tournament_id: tournament_id.into(),
                player,
                event_type: RegistrationEventType::PlayerRegistered,
            };

            publish_registration_event(event);
        }

        // Get tournament name for notification message
        if let Ok(Some(tournament)) = tournaments::get_by_id(&state.db, tournament_id).await {
            // Publish registration confirmed notification to the user
            let notification = UserNotification {
                id: ID::from(Uuid::new_v4().to_string()),
                user_id: ID::from(user_id.to_string()),
                notification_type: NotificationType::RegistrationConfirmed,
                title: TITLE_REGISTRATION_CONFIRMED.to_string(),
                message: format!("You are registered for {}", tournament.name),
                tournament_id: Some(ID::from(tournament_id.to_string())),
                created_at: Utc::now(),
            };

            publish_user_notification(notification);
        }

        Ok(tournament_registration)
    }

    /// Check in a player for a tournament with optional auto-assignment
    async fn check_in_player(
        &self,
        ctx: &Context<'_>,
        input: CheckInPlayerInput,
    ) -> Result<CheckInResponse> {
        use crate::auth::permissions::require_role;

        // Require manager role for check-in
        let manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;
        let user_id = Uuid::parse_str(input.user_id.as_str()).gql_err("Invalid user ID")?;
        let manager_id = Uuid::parse_str(manager.id.as_str()).gql_err("Invalid manager ID")?;

        // Get the registration (read before transaction)
        let registration =
            tournament_registrations::get_by_tournament_and_user(&state.db, tournament_id, user_id)
                .await?
                .ok_or_else(|| {
                    async_graphql::Error::new("Player not registered for this tournament")
                })?;

        // Check current status
        let current_status: RegistrationStatus = registration.status.clone().into();
        if current_status != RegistrationStatus::Registered {
            return Err(async_graphql::Error::new(format!(
                "Player cannot be checked in from status: {:?}",
                current_status
            )));
        }

        // Begin transaction for all write operations
        let mut tx = state
            .db
            .begin()
            .await
            .gql_err("Database operation failed")?;

        // Update status to CHECKED_IN
        tournament_registrations::update_status(&mut *tx, tournament_id, user_id, "checked_in")
            .await
            .gql_err("Database operation failed")?;

        // Get updated registration
        let updated_registration =
            tournament_registrations::get_by_tournament_and_user(&mut *tx, tournament_id, user_id)
                .await
                .gql_err("Database operation failed")?
                .ok_or_else(|| async_graphql::Error::new("Failed to get updated registration"))?;

        // Apply early bird bonus if requested
        if input.grant_early_bird_bonus.unwrap_or(false) {
            let tournament = tournaments::get_by_id(&state.db, tournament_id)
                .await?
                .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

            if let Some(bonus_chips) = tournament.early_bird_bonus_chips {
                tournament_entries::apply_early_bird_bonus(
                    &mut *tx,
                    tournament_id,
                    user_id,
                    bonus_chips,
                )
                .await
                .gql_err("Database operation failed")?;
            }
        }

        // Auto-assign to table if requested
        let auto_assign = input.auto_assign.unwrap_or(true);
        let strategy = input
            .assignment_strategy
            .unwrap_or(AssignmentStrategy::Balanced);

        let mut seat_assignment = None;
        let mut message = String::from("Player checked in successfully");

        if auto_assign && strategy != AssignmentStrategy::Manual {
            // Get tournament tables
            let tables = club_tables::list_assigned_to_tournament(&state.db, tournament_id).await?;

            if !tables.is_empty() {
                // Get current assignments to find best table
                let current_assignments =
                    table_seat_assignments::list_current_for_tournament(&mut *tx, tournament_id)
                        .await
                        .gql_err("Database operation failed")?;

                // Count players per table
                let mut table_counts: std::collections::HashMap<Uuid, usize> =
                    std::collections::HashMap::new();
                for assignment in &current_assignments {
                    *table_counts.entry(assignment.club_table_id).or_insert(0) += 1;
                }

                // Find best table based on strategy
                let target_table = match strategy {
                    AssignmentStrategy::Balanced => {
                        // Find table with minimum players
                        tables
                            .iter()
                            .min_by_key(|table| table_counts.get(&table.id).unwrap_or(&0))
                            .ok_or_else(|| async_graphql::Error::new("No tables available"))?
                    }
                    AssignmentStrategy::Random => {
                        // Random table selection
                        use rand::seq::IndexedRandom;
                        tables
                            .choose(&mut rand::rng())
                            .ok_or_else(|| async_graphql::Error::new("No tables available"))?
                    }
                    AssignmentStrategy::Sequential => {
                        // Fill tables in order, find first non-full table
                        tables
                            .iter()
                            .find(|table| {
                                let count = table_counts.get(&table.id).unwrap_or(&0);
                                *count < table.max_seats as usize
                            })
                            .ok_or_else(|| async_graphql::Error::new("All tables are full"))?
                    }
                    _ => unreachable!(),
                };

                // Get all occupied seats in one query and find available ones
                let occupied_seats: std::collections::HashSet<i32> =
                    table_seat_assignments::get_occupied_seats(&mut *tx, target_table.id)
                        .await
                        .gql_err("Database operation failed")?
                        .into_iter()
                        .collect();
                let available_seats: Vec<i32> = (1..=target_table.max_seats)
                    .filter(|seat| !occupied_seats.contains(seat))
                    .collect();

                if !available_seats.is_empty() {
                    // Pick a random seat from available ones
                    let random_index = rand::rng().random_range(0..available_seats.len());
                    let seat_num = available_seats[random_index];

                    // Assign player to this randomly selected seat
                    let create_data = CreateSeatAssignment {
                        tournament_id,
                        club_table_id: target_table.id,
                        user_id,
                        seat_number: seat_num,
                        stack_size: None, // Will be set when tournament starts
                        assigned_by: Some(manager_id),
                        notes: Some(format!(
                            "Auto-assigned on check-in using {:?} strategy",
                            strategy
                        )),
                    };

                    let assignment_row = table_seat_assignments::create(&mut *tx, create_data)
                        .await
                        .gql_err("Database operation failed")?;

                    seat_assignment = Some(SeatAssignment {
                        id: assignment_row.id.into(),
                        tournament_id: assignment_row.tournament_id.into(),
                        club_table_id: assignment_row.club_table_id.into(),
                        user_id: assignment_row.user_id.into(),
                        seat_number: assignment_row.seat_number,
                        stack_size: assignment_row.stack_size,
                        is_current: assignment_row.is_current,
                        assigned_at: assignment_row.assigned_at,
                        unassigned_at: assignment_row.unassigned_at,
                        assigned_by: assignment_row.assigned_by.map(|id| id.into()),
                        notes: assignment_row.notes,
                    });

                    message = format!(
                        "Player checked in and assigned to Table {}, Seat {}",
                        target_table.table_number, seat_num
                    );
                } else {
                    // No available seats
                    message =
                        "Player checked in but no seats available for auto-assignment".to_string();
                }
            } else {
                message = "Player checked in but no tables assigned to tournament yet".to_string();
            }
        }

        // Commit transaction
        tx.commit().await.gql_err("Database operation failed")?;

        // Emit seating event after commit (side effects should happen after successful commit)
        if let Some(ref assignment) = seat_assignment {
            let user_loader = ctx.data::<DataLoader<UserLoader>>()?;
            if let Some(user_row) = user_loader
                .load_one(user_id)
                .await
                .gql_err("Database operation failed")?
            {
                let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;
                let event = SeatingChangeEvent {
                    event_type: SeatingEventType::PlayerAssigned,
                    tournament_id: tournament_id.into(),
                    club_id: club_id.into(),
                    affected_assignment: Some(assignment.clone()),
                    affected_player: Some(User {
                        id: user_row.id.into(),
                        email: user_row.email,
                        username: user_row.username,
                        first_name: user_row.first_name,
                        last_name: user_row.last_name,
                        phone: user_row.phone,
                        is_active: user_row.is_active,
                        role: Role::from(user_row.role),
                    }),
                    message: message.clone(),
                    timestamp: chrono::Utc::now(),
                };
                publish_seating_event(event);
            }
        }

        Ok(CheckInResponse {
            registration: TournamentRegistration {
                id: updated_registration.id.into(),
                tournament_id: updated_registration.tournament_id.into(),
                user_id: updated_registration.user_id.into(),
                registration_time: updated_registration.registration_time,
                status: updated_registration.status.into(),
                notes: updated_registration.notes,
            },
            seat_assignment,
            message,
        })
    }
}
