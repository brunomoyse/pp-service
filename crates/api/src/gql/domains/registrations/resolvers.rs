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
    AssignmentStrategy, CheckInPlayerInput, CheckInResponse, NotificationType,
    PlayerRegistrationEvent, RegisterForTournamentInput, RegistrationEventType, SeatAssignment,
    SeatingChangeEvent, SeatingEventType, TournamentPlayer, TournamentRegistration, User,
    UserNotification, TITLE_REGISTRATION_CONFIRMED,
};
use crate::state::AppState;
use infra::repos::{
    tournament_registrations, tournament_registrations::CreateTournamentRegistration, tournaments,
    users,
};

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
                players.push(TournamentPlayer {
                    registration: registration.into(),
                    user: user_row.clone().into(),
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
            .map(TournamentRegistration::from)
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
                // Manager is registering another user
                Uuid::parse_str(target_user_id.as_str()).gql_err("Invalid target user ID")?
            }
            None => {
                // User registering themselves
                Uuid::parse_str(authenticated_user.id.as_str()).gql_err("Invalid user ID")?
            }
        };

        let create_data = CreateTournamentRegistration {
            tournament_id,
            user_id,
            notes: input.notes.clone(),
        };

        let row = tournament_registrations::create(&state.db, create_data).await?;

        let tournament_registration: TournamentRegistration = row.into();

        // Emit subscription event
        if let Some(user_row) = users::get_by_id(&state.db, user_id).await? {
            let user: User = user_row.into();

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

        Ok(CheckInResponse {
            registration: result.updated_registration.into(),
            seat_assignment,
            message: result.message,
        })
    }
}
