use async_graphql::{dataloader::DataLoader, Context, Object, Result, ID};
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

use crate::gql::common::helpers::get_club_id_for_tournament;
use crate::gql::error::ResultExt;
use crate::gql::loaders::UserLoader;
use crate::gql::subscriptions::{
    publish_registration_event, publish_seating_event, publish_user_notification,
};
use crate::gql::types::{
    AssignmentStrategy, CancelRegistrationInput, CancelRegistrationResponse, CheckInPlayerInput,
    CheckInResponse, NotificationType, PaginatedResponse, PaginationInput, PlayerRegistrationEvent,
    RegisterForTournamentInput, RegistrationEventType, SeatAssignment, SeatingChangeEvent,
    SeatingEventType, SelfCheckInInput, SelfCheckInResponse, TournamentPlayer,
    TournamentRegistration, User, UserNotification, TITLE_REGISTRATION_CONFIRMED, TITLE_WAITLISTED,
    TITLE_WAITLIST_PROMOTED,
};
use crate::state::AppState;
use infra::repos::{
    table_seat_assignments, tournament_registrations,
    tournament_registrations::CreateTournamentRegistration, tournaments, users,
};

#[derive(Default)]
pub struct RegistrationQuery;

#[Object]
impl RegistrationQuery {
    async fn tournament_players(
        &self,
        ctx: &Context<'_>,
        tournament_id: Uuid,
        pagination: Option<PaginationInput>,
    ) -> Result<PaginatedResponse<TournamentPlayer>> {
        use crate::auth::Claims;

        let _claims = ctx
            .data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;

        let state = ctx.data::<AppState>()?;

        let page_params = pagination.unwrap_or(PaginationInput {
            limit: Some(50),
            offset: Some(0),
        });
        let limit_offset = page_params.to_limit_offset();

        // Fetch registrations and total count in parallel
        let (registrations, total_count) = tokio::try_join!(
            tournament_registrations::list_by_tournament_paginated(
                &state.db,
                tournament_id,
                limit_offset
            ),
            tournament_registrations::count_by_tournament(&state.db, tournament_id)
        )?;

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
                players.push(TournamentPlayer {
                    registration: registration.into(),
                    user: user_row.clone().into(),
                });
            }
        }

        let page_size = players.len() as i32;
        let offset = limit_offset.offset as i32;
        let has_next_page = (offset + page_size) < total_count as i32;

        Ok(PaginatedResponse {
            items: players,
            total_count: total_count as i32,
            page_size,
            offset,
            has_next_page,
        })
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
            .map(TournamentRegistration::from)
            .collect())
    }
}

#[derive(Default)]
pub struct RegistrationMutation;

#[Object]
impl RegistrationMutation {
    /// Register a user for a tournament. Auto-waitlists if tournament is full.
    async fn register_for_tournament(
        &self,
        ctx: &Context<'_>,
        input: RegisterForTournamentInput,
    ) -> Result<TournamentRegistration> {
        use crate::auth::permissions::require_manager_if;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        // Check permissions: if registering another user, require club manager
        let is_manager_registration = input.user_id.is_some();
        let authenticated_user = if is_manager_registration {
            let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;
            use crate::auth::permissions::require_club_manager;
            Some(require_club_manager(ctx, club_id).await?)
        } else {
            require_manager_if(ctx, false, "user_id").await?
        }
        .ok_or_else(|| async_graphql::Error::new("You must be logged in to perform this action"))?;

        // Determine which user to register
        let user_id = match input.user_id {
            Some(target_user_id) => {
                Uuid::parse_str(target_user_id.as_str()).gql_err("Invalid target user ID")?
            }
            None => Uuid::parse_str(authenticated_user.id.as_str()).gql_err("Invalid user ID")?,
        };

        // Use a transaction with row-level locking to prevent race conditions
        let mut tx = state.db.begin().await?;

        // Lock the tournament row to prevent concurrent registrations from racing
        let tournament = sqlx::query_as::<_, infra::models::TournamentRow>(
            "SELECT id, club_id, name, description, start_time, end_time, buy_in_cents, rake_cents, seat_cap, live_status, early_bird_bonus_chips, late_registration_level, created_at, updated_at FROM tournaments WHERE id = $1 FOR UPDATE",
        )
        .bind(tournament_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        // Only allow registration during REGISTRATION_OPEN or LATE_REGISTRATION
        {
            use infra::repos::tournaments::TournamentLiveStatus;
            match tournament.live_status {
                TournamentLiveStatus::RegistrationOpen | TournamentLiveStatus::LateRegistration => { /* allowed */
                }
                _ => {
                    return Err(async_graphql::Error::new(
                        "Registration is not open for this tournament",
                    ));
                }
            }
        }

        // Determine status based on seat capacity
        let is_waitlisted = if let Some(seat_cap) = tournament.seat_cap {
            let confirmed_count =
                tournament_registrations::count_confirmed_by_tournament(&mut *tx, tournament_id)
                    .await?;
            confirmed_count >= seat_cap as i64
        } else {
            false
        };

        let status = if is_waitlisted {
            Some("waitlisted".to_string())
        } else {
            None // defaults to 'registered'
        };

        let create_data = CreateTournamentRegistration {
            tournament_id,
            user_id,
            notes: input.notes.clone(),
            status,
        };

        let row = tournament_registrations::create(&mut *tx, create_data).await?;

        tx.commit().await?;

        let tournament_registration: TournamentRegistration = row.into();

        // Emit subscription event
        if let Some(user_row) = users::get_by_id(&state.db, user_id).await? {
            let user: User = user_row.into();

            let player = TournamentPlayer {
                registration: tournament_registration.clone(),
                user,
            };

            let event_type = if is_waitlisted {
                RegistrationEventType::PlayerWaitlisted
            } else {
                RegistrationEventType::PlayerRegistered
            };

            let event = PlayerRegistrationEvent {
                tournament_id: tournament_id.into(),
                player,
                event_type,
            };

            publish_registration_event(event);
        }

        // Log activity
        let action = if is_waitlisted {
            "waitlisted"
        } else {
            "registered"
        };
        let db = state.db.clone();
        tokio::spawn(async move {
            crate::gql::domains::activity_log::log_and_publish(
                &db,
                tournament_id,
                "registration",
                action,
                Some(user_id),
                Some(user_id),
                serde_json::json!({}),
            )
            .await;
        });

        // Publish notification
        if is_waitlisted {
            // Get waitlist position for the notification
            let position =
                tournament_registrations::get_waitlist_position(&state.db, tournament_id, user_id)
                    .await
                    .unwrap_or(None)
                    .unwrap_or(0);

            let notification = UserNotification {
                id: ID::from(Uuid::new_v4().to_string()),
                user_id: ID::from(user_id.to_string()),
                notification_type: NotificationType::RegistrationConfirmed,
                title: TITLE_WAITLISTED.to_string(),
                message: format!(
                    "You are on the waitlist for {} (position {})",
                    tournament.name, position
                ),
                tournament_id: Some(ID::from(tournament_id.to_string())),
                created_at: Utc::now(),
            };
            publish_user_notification(notification);
        } else {
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

            // Send confirmation email (fire-and-forget)
            if let Some(email_service) = state.email_service() {
                if let Some(user_row) = users::get_by_id(&state.db, user_id).await? {
                    let locale =
                        crate::services::email_service::Locale::from_str_lossy(&user_row.locale);
                    crate::services::email_service::spawn_email(
                        email_service.clone(),
                        user_row.email,
                        user_row.first_name,
                        crate::services::email_service::EmailType::RegistrationConfirmed {
                            tournament_name: tournament.name.clone(),
                            locale,
                        },
                    );
                }
            }
        }

        Ok(tournament_registration)
    }

    /// Cancel a registration. If the player was confirmed (not waitlisted), promotes the next waitlisted player.
    async fn cancel_registration(
        &self,
        ctx: &Context<'_>,
        input: CancelRegistrationInput,
    ) -> Result<CancelRegistrationResponse> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;
        let user_id = Uuid::parse_str(input.user_id.as_str()).gql_err("Invalid user ID")?;

        // Get the current user's claims
        let claims = ctx
            .data::<crate::auth::Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;
        let authenticated_user_id =
            Uuid::parse_str(&claims.sub).gql_err("Invalid authenticated user ID")?;

        // Determine if the caller is a manager
        let is_manager = if authenticated_user_id != user_id {
            let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;
            require_club_manager(ctx, club_id).await?;
            true
        } else {
            false
        };

        // Get current registration
        let registration =
            tournament_registrations::get_by_tournament_and_user(&state.db, tournament_id, user_id)
                .await?
                .ok_or_else(|| async_graphql::Error::new("Registration not found"))?;

        // Players can only cancel from registered or waitlisted status;
        // managers can cancel any status (e.g. to fix mistakes)
        if !is_manager && registration.status != "registered" && registration.status != "waitlisted"
        {
            return Err(async_graphql::Error::new(format!(
                "Cannot cancel registration with status: {}",
                registration.status
            )));
        }

        let was_confirmed = registration.status != "waitlisted";
        let was_seated = registration.status == "seated";

        // Use a transaction to ensure status update and seat clearing are atomic
        let mut tx = state
            .db
            .begin()
            .await
            .gql_err("Failed to begin transaction")?;

        // Update status to cancelled
        tournament_registrations::update_status(&mut *tx, tournament_id, user_id, "cancelled")
            .await?;

        // If the player was seated, clear their seat assignment
        if was_seated {
            table_seat_assignments::unassign_current_seat(&mut *tx, tournament_id, user_id, None)
                .await?;
        }

        tx.commit().await.gql_err("Failed to commit transaction")?;

        // Get updated registration
        let updated_row =
            tournament_registrations::get_by_tournament_and_user(&state.db, tournament_id, user_id)
                .await?
                .ok_or_else(|| async_graphql::Error::new("Failed to get updated registration"))?;

        let updated_registration: TournamentRegistration = updated_row.into();

        // Emit unregistered event
        if let Some(user_row) = users::get_by_id(&state.db, user_id).await? {
            let user: User = user_row.into();
            let player = TournamentPlayer {
                registration: updated_registration.clone(),
                user,
            };
            let event = PlayerRegistrationEvent {
                tournament_id: tournament_id.into(),
                player,
                event_type: RegistrationEventType::PlayerUnregistered,
            };
            publish_registration_event(event);
        }

        // If the cancelled player was confirmed (not waitlisted), promote the next waitlisted player
        let mut promoted_player: Option<TournamentPlayer> = None;
        if was_confirmed {
            if let Ok(Some(promotion)) =
                super::service::promote_next_waitlisted(&state.db, tournament_id).await
            {
                let promoted_user_id = promotion.promoted_registration.user_id;
                let promoted_registration: TournamentRegistration =
                    promotion.promoted_registration.into();

                if let Some(user_row) = users::get_by_id(&state.db, promoted_user_id).await? {
                    let user: User = user_row.into();

                    let player = TournamentPlayer {
                        registration: promoted_registration,
                        user,
                    };

                    // Emit promotion event
                    let event = PlayerRegistrationEvent {
                        tournament_id: tournament_id.into(),
                        player: player.clone(),
                        event_type: RegistrationEventType::PlayerPromoted,
                    };
                    publish_registration_event(event);

                    // Notify the promoted player
                    if let Ok(Some(tournament)) =
                        tournaments::get_by_id(&state.db, tournament_id).await
                    {
                        let notification = UserNotification {
                            id: ID::from(Uuid::new_v4().to_string()),
                            user_id: ID::from(promoted_user_id.to_string()),
                            notification_type: NotificationType::WaitlistPromoted,
                            title: TITLE_WAITLIST_PROMOTED.to_string(),
                            message: format!(
                                "A spot opened up! You are now registered for {}",
                                tournament.name
                            ),
                            tournament_id: Some(ID::from(tournament_id.to_string())),
                            created_at: Utc::now(),
                        };
                        publish_user_notification(notification);

                        // Send waitlist promotion email (fire-and-forget)
                        if let Some(email_service) = state.email_service() {
                            if let Some(promoted_user_row) =
                                users::get_by_id(&state.db, promoted_user_id).await?
                            {
                                let locale = crate::services::email_service::Locale::from_str_lossy(
                                    &promoted_user_row.locale,
                                );
                                crate::services::email_service::spawn_email(
                                    email_service.clone(),
                                    promoted_user_row.email,
                                    promoted_user_row.first_name,
                                    crate::services::email_service::EmailType::WaitlistPromoted {
                                        tournament_name: tournament.name.clone(),
                                        locale,
                                    },
                                );
                            }
                        }
                    }

                    promoted_player = Some(player);
                }
            }
        }

        // Log activity
        {
            let db = state.db.clone();
            let promoted = promoted_player.is_some();
            tokio::spawn(async move {
                crate::gql::domains::activity_log::log_and_publish(
                    &db,
                    tournament_id,
                    "registration",
                    "cancelled",
                    Some(authenticated_user_id),
                    Some(user_id),
                    serde_json::json!({}),
                )
                .await;
                if promoted {
                    // The promoted player activity is also logged
                    crate::gql::domains::activity_log::log_and_publish(
                        &db,
                        tournament_id,
                        "registration",
                        "promoted",
                        None,
                        None,
                        serde_json::json!({}),
                    )
                    .await;
                }
            });
        }

        Ok(CancelRegistrationResponse {
            registration: updated_registration,
            promoted_player,
        })
    }

    /// Self check-in: a player scans a tournament QR code and checks themselves in.
    /// If not registered, registers first then checks in.
    async fn self_check_in(
        &self,
        ctx: &Context<'_>,
        input: SelfCheckInInput,
    ) -> Result<SelfCheckInResponse> {
        use crate::auth::Claims;

        let state = ctx.data::<AppState>()?;

        let claims = ctx
            .data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;
        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let params = super::service::SelfCheckInParams {
            tournament_id,
            user_id,
            auto_assign: true,
            assignment_strategy: AssignmentStrategy::Balanced,
        };

        let result = super::service::self_check_in(&state.db, params)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        let seat_assignment: Option<SeatAssignment> =
            result.seat_assignment.map(SeatAssignment::from);

        // Publish registration event if newly registered
        if result.was_registered {
            if let Some(user_row) = users::get_by_id(&state.db, user_id).await? {
                let user: User = user_row.into();
                let registration: TournamentRegistration =
                    result.updated_registration.clone().into();

                let player = TournamentPlayer {
                    registration: registration.clone(),
                    user,
                };

                let event = PlayerRegistrationEvent {
                    tournament_id: tournament_id.into(),
                    player,
                    event_type: RegistrationEventType::PlayerRegistered,
                };
                publish_registration_event(event);
            }
        }

        // Publish seating event if auto-assigned
        if let Some(ref assignment) = seat_assignment {
            let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;
            let user_loader = ctx.data::<DataLoader<UserLoader>>()?;
            if let Some(user_row) = user_loader
                .load_one(user_id)
                .await
                .gql_err("Database operation failed")?
            {
                let event = SeatingChangeEvent {
                    event_type: SeatingEventType::PlayerAssigned,
                    tournament_id: tournament_id.into(),
                    club_id: club_id.into(),
                    affected_assignment: Some(assignment.clone()),
                    affected_player: Some(user_row.into()),
                    message: result.message.clone(),
                    timestamp: chrono::Utc::now(),
                };
                publish_seating_event(event);
            }
        }

        // Log activity
        {
            let db = state.db.clone();
            let auto_seated = seat_assignment.is_some();
            tokio::spawn(async move {
                crate::gql::domains::activity_log::log_and_publish(
                    &db,
                    tournament_id,
                    "registration",
                    "self_check_in",
                    Some(user_id),
                    Some(user_id),
                    serde_json::json!({"auto_seated": auto_seated}),
                )
                .await;
            });
        }

        Ok(SelfCheckInResponse {
            registration: result.updated_registration.into(),
            seat_assignment,
            message: result.message,
            was_registered: result.was_registered,
        })
    }

    /// Check in a player for a tournament with optional auto-assignment
    async fn check_in_player(
        &self,
        ctx: &Context<'_>,
        input: CheckInPlayerInput,
    ) -> Result<CheckInResponse> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        // Auth
        let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;
        let manager = require_club_manager(ctx, club_id).await?;
        let user_id = Uuid::parse_str(input.user_id.as_str()).gql_err("Invalid user ID")?;
        let manager_id = Uuid::parse_str(manager.id.as_str()).gql_err("Invalid manager ID")?;

        // Delegate to service
        let params = super::service::CheckInParams {
            tournament_id,
            user_id,
            manager_id,
            auto_assign: input.auto_assign.unwrap_or(true),
            assignment_strategy: input
                .assignment_strategy
                .unwrap_or(AssignmentStrategy::Balanced),
            grant_early_bird_bonus: input.grant_early_bird_bonus.unwrap_or(false),
        };

        let result = super::service::check_in_player(&state.db, params)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Convert to GQL types
        let seat_assignment: Option<SeatAssignment> =
            result.seat_assignment.map(SeatAssignment::from);

        // Publish seating event after successful commit
        if let Some(ref assignment) = seat_assignment {
            let user_loader = ctx.data::<DataLoader<UserLoader>>()?;
            if let Some(user_row) = user_loader
                .load_one(user_id)
                .await
                .gql_err("Database operation failed")?
            {
                let event = SeatingChangeEvent {
                    event_type: SeatingEventType::PlayerAssigned,
                    tournament_id: tournament_id.into(),
                    club_id: club_id.into(),
                    affected_assignment: Some(assignment.clone()),
                    affected_player: Some(user_row.into()),
                    message: result.message.clone(),
                    timestamp: chrono::Utc::now(),
                };
                publish_seating_event(event);
            }
        }

        // Log activity
        {
            let db = state.db.clone();
            let auto_seated = seat_assignment.is_some();
            tokio::spawn(async move {
                crate::gql::domains::activity_log::log_and_publish(
                    &db,
                    tournament_id,
                    "registration",
                    "check_in",
                    Some(manager_id),
                    Some(user_id),
                    serde_json::json!({"auto_seated": auto_seated}),
                )
                .await;
            });
        }

        Ok(CheckInResponse {
            registration: result.updated_registration.into(),
            seat_assignment,
            message: result.message,
        })
    }
}
