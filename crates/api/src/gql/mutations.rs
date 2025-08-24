use async_graphql::{Context, InputObject, Object, Result, ID};

use super::subscriptions::{publish_registration_event, publish_seating_event};
use super::types::{
    AssignPlayerToSeatInput, AssignTableToTournamentInput, AuthPayload, BalanceTablesInput,
    CreateOAuthClientInput, CreateOAuthClientResponse, DealType, EnterTournamentResultsInput,
    EnterTournamentResultsResponse, MovePlayerInput, OAuthCallbackInput, OAuthClient,
    OAuthUrlResponse, PlayerDeal, PlayerDealInput, PlayerPositionInput, PlayerRegistrationEvent,
    RegisterForTournamentInput, Role, SeatAssignment, SeatingChangeEvent, SeatingEventType,
    Tournament, TournamentPlayer, TournamentRegistration, TournamentResult, TournamentState,
    TournamentTable, UpdateStackSizeInput, UpdateTournamentStateInput, UpdateTournamentStatusInput,
    User, UserLoginInput, UserRegistrationInput,
};
use crate::auth::{
    custom_oauth::CustomOAuthService, password::PasswordService, permissions::require_admin_if,
    Claims, OAuthProvider,
};
use crate::state::AppState;
use infra::models::TournamentRow;
use infra::repos::{
    ClubTableRepo, CreatePlayerDeal, CreateSeatAssignment, CreateTournamentRegistration,
    CreateTournamentResult, PayoutTemplateRepo, PlayerDealRepo, TableSeatAssignmentRepo,
    TournamentLiveStatus, TournamentRegistrationRepo, TournamentRepo, TournamentResultRepo,
    UpdateSeatAssignment, UpdateTournamentState, UserRepo,
};
use rand::{distributions::Alphanumeric, Rng};
use serde_json;
use uuid::Uuid;

// Helper function to get club_id from tournament_id for events
async fn get_club_id_for_tournament(db: &infra::db::Db, tournament_id: Uuid) -> Result<Uuid> {
    let tournament_repo = TournamentRepo::new(db.clone());
    let tournament = tournament_repo
        .get(tournament_id)
        .await?
        .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;
    Ok(tournament.club_id)
}

pub struct MutationRoot;

#[derive(InputObject)]
pub struct CreateTournamentInput {
    pub title: String,
    pub club_id: ID,
}

#[Object]
impl MutationRoot {
    /// Initialize tournament clock
    async fn create_tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<crate::gql::types::TournamentClock> {
        let mutation = crate::gql::tournament_clock::TournamentClockMutation;
        mutation.create_tournament_clock(ctx, tournament_id).await
    }

    /// Start tournament clock
    async fn start_tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<crate::gql::types::TournamentClock> {
        let mutation = crate::gql::tournament_clock::TournamentClockMutation;
        mutation.start_tournament_clock(ctx, tournament_id).await
    }

    /// Pause tournament clock
    async fn pause_tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<crate::gql::types::TournamentClock> {
        let mutation = crate::gql::tournament_clock::TournamentClockMutation;
        mutation.pause_tournament_clock(ctx, tournament_id).await
    }

    /// Resume tournament clock
    async fn resume_tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<crate::gql::types::TournamentClock> {
        let mutation = crate::gql::tournament_clock::TournamentClockMutation;
        mutation.resume_tournament_clock(ctx, tournament_id).await
    }

    /// Manually advance to next level
    async fn advance_tournament_level(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<crate::gql::types::TournamentClock> {
        let mutation = crate::gql::tournament_clock::TournamentClockMutation;
        mutation.advance_tournament_level(ctx, tournament_id).await
    }

    /// Manually revert to previous level
    async fn revert_tournament_level(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<crate::gql::types::TournamentClock> {
        let mutation = crate::gql::tournament_clock::TournamentClockMutation;
        mutation.revert_tournament_level(ctx, tournament_id).await
    }

    /// Minimal example mutation creating a tournament (stub).
    /// Replace with an INSERT via sqlx later.
    async fn create_tournament(
        &self,
        ctx: &Context<'_>,
        input: CreateTournamentInput,
    ) -> Result<Tournament> {
        let _state = ctx.data::<AppState>()?;
        // Example: persist with sqlx here using _state.db

        Ok(Tournament {
            id: "new_tournament_id".into(),
            title: input.title,
            description: None,
            club_id: input.club_id,
            start_time: chrono::Utc::now(),
            end_time: None,
            buy_in_cents: 0,
            seat_cap: None,
            status: crate::gql::types::TournamentStatus::Upcoming,
            live_status: crate::gql::types::TournamentLiveStatus::NotStarted,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    }

    /// Register a user for a tournament.
    async fn register_for_tournament(
        &self,
        ctx: &Context<'_>,
        input: RegisterForTournamentInput,
    ) -> Result<TournamentRegistration> {
        let state = ctx.data::<AppState>()?;
        let registration_repo = TournamentRegistrationRepo::new(state.db.clone());
        let user_repo = UserRepo::new(state.db.clone());

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
                Uuid::parse_str(target_user_id.as_str()).map_err(|e| {
                    async_graphql::Error::new(format!("Invalid target user ID: {}", e))
                })?
            }
            None => {
                // User registering themselves
                Uuid::parse_str(authenticated_user.id.as_str())
                    .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?
            }
        };

        let tournament_id = Uuid::parse_str(input.tournament_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament_id: {}", e)))?;

        let create_data = CreateTournamentRegistration {
            tournament_id,
            user_id,
            notes: input.notes.clone(),
        };

        let row = registration_repo.create(create_data).await?;

        let tournament_registration = TournamentRegistration {
            id: row.id.into(),
            tournament_id: row.tournament_id.into(),
            user_id: row.user_id.into(),
            registration_time: row.registration_time,
            status: row.status.clone(),
            notes: row.notes.clone(),
        };

        // Emit subscription event
        if let Some(user_row) = user_repo.get_by_id(user_id).await? {
            let user = User {
                id: user_row.id.into(),
                email: user_row.email,
                username: user_row.username,
                first_name: user_row.first_name,
                last_name: user_row.last_name,
                phone: user_row.phone,
                is_active: user_row.is_active,
                role: crate::gql::types::Role::from(user_row.role),
            };

            let player = TournamentPlayer {
                registration: tournament_registration.clone(),
                user,
            };

            let event = PlayerRegistrationEvent {
                tournament_id: tournament_id.into(),
                player,
                event_type: "player_registered".to_string(),
            };

            publish_registration_event(event);
        }

        Ok(tournament_registration)
    }

    /// Get OAuth authorization URL
    async fn get_oauth_url(&self, ctx: &Context<'_>, provider: String) -> Result<OAuthUrlResponse> {
        let state = ctx.data::<AppState>()?;

        let provider = match provider.as_str() {
            "google" => OAuthProvider::Google,
            "custom" => OAuthProvider::Custom,
            _ => return Err(async_graphql::Error::new("Invalid OAuth provider")),
        };

        let (auth_url, csrf_token) = state
            .oauth_service()
            .get_authorize_url(provider)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(OAuthUrlResponse {
            auth_url,
            csrf_token,
        })
    }

    /// Complete OAuth authentication flow
    async fn oauth_callback(
        &self,
        ctx: &Context<'_>,
        input: OAuthCallbackInput,
    ) -> Result<AuthPayload> {
        let state = ctx.data::<AppState>()?;

        let provider = match input.provider.as_str() {
            "google" => OAuthProvider::Google,
            "custom" => OAuthProvider::Custom,
            _ => return Err(async_graphql::Error::new("Invalid OAuth provider")),
        };

        let oauth_user = match provider {
            OAuthProvider::Custom => {
                CustomOAuthService::exchange_code_for_user_info(state, input.code)
                    .await
                    .map_err(|e| async_graphql::Error::new(e.to_string()))?
            }
            _ => state
                .oauth_service()
                .exchange_code_for_user_info(provider, input.code)
                .await
                .map_err(|e| async_graphql::Error::new(e.to_string()))?,
        };

        // Check if user exists, if not create them
        let user_id = match find_user_by_email(state, &oauth_user.email).await? {
            Some(existing_user) => Uuid::parse_str(existing_user.id.as_str())
                .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?,
            None => create_user_from_oauth(state, &oauth_user, &input.provider).await?,
        };

        // Generate JWT token
        let token = state
            .jwt_service()
            .create_token(user_id, oauth_user.email.clone())
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Get user info for response
        let user = get_user_by_id(state, user_id).await?;

        Ok(AuthPayload { token, user })
    }

    /// Validate token and get current user
    async fn me(&self, ctx: &Context<'_>) -> Result<User> {
        // Extract claims from request extensions (set by middleware)
        let claims = ctx.data::<Claims>().map_err(|_| {
            async_graphql::Error::new("You must be logged in to perform this action")
        })?;

        let state = ctx.data::<AppState>()?;
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;

        get_user_by_id(state, user_id).await
    }

    /// Create OAuth client (admin only - you should add proper authorization)
    async fn create_oauth_client(
        &self,
        ctx: &Context<'_>,
        input: CreateOAuthClientInput,
    ) -> Result<CreateOAuthClientResponse> {
        let state = ctx.data::<AppState>()?;

        // Generate client credentials
        let client_id = generate_client_id();
        let client_secret = generate_client_secret();

        let scopes = input.scopes.unwrap_or_else(|| vec!["read".to_string()]);

        let row = sqlx::query!(
            r#"
            INSERT INTO oauth_clients (client_id, client_secret, name, redirect_uris, scopes)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
            client_id,
            client_secret,
            input.name,
            &input.redirect_uris,
            &scopes
        )
        .fetch_one(&state.db)
        .await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        let client = OAuthClient {
            id: row.id.into(),
            client_id: client_id.clone(),
            name: input.name,
            redirect_uris: input.redirect_uris,
            scopes,
            is_active: true,
        };

        Ok(CreateOAuthClientResponse {
            client,
            client_secret,
        })
    }

    /// Register user with password (for custom OAuth)
    async fn register_user(&self, ctx: &Context<'_>, input: UserRegistrationInput) -> Result<User> {
        let state = ctx.data::<AppState>()?;

        // Validate password strength
        PasswordService::validate_password_strength(&input.password)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Hash password
        let password_hash = PasswordService::hash_password(&input.password)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Check if user already exists
        let existing_user = sqlx::query!("SELECT id FROM users WHERE email = $1", input.email)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        if existing_user.is_some() {
            return Err(async_graphql::Error::new(
                "User with this email already exists",
            ));
        }

        // Create user
        let row = sqlx::query!(
            r#"
            INSERT INTO users (email, first_name, last_name, username, password_hash)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
            input.email,
            input.first_name,
            input.last_name,
            input.username,
            password_hash
        )
        .fetch_one(&state.db)
        .await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(User {
            id: row.id.into(),
            email: input.email,
            username: input.username,
            first_name: input.first_name,
            last_name: Some(input.last_name),
            phone: None,
            is_active: true,
            role: crate::gql::types::Role::Player,
        })
    }

    /// Login user with password (returns JWT token)
    async fn login_user(&self, ctx: &Context<'_>, input: UserLoginInput) -> Result<AuthPayload> {
        let state = ctx.data::<AppState>()?;

        // Find user by email
        let user_row = sqlx::query!(
            "SELECT id, email, username, first_name, last_name, phone, is_active, password_hash, role FROM users WHERE email = $1",
            input.email
        )
        .fetch_optional(&state.db)
        .await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        let user_row = match user_row {
            Some(row) => row,
            None => return Err(async_graphql::Error::new("Invalid credentials")),
        };

        // Verify password
        if let Some(ref password_hash) = user_row.password_hash {
            if !PasswordService::verify_password(&input.password, password_hash)
                .map_err(|e| async_graphql::Error::new(e.to_string()))?
            {
                return Err(async_graphql::Error::new("Invalid credentials"));
            }
        } else {
            return Err(async_graphql::Error::new("User has no password set"));
        }

        // Generate JWT token
        let token = state
            .jwt_service()
            .create_token(user_row.id, user_row.email.clone())
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        let user = User {
            id: user_row.id.into(),
            email: user_row.email,
            username: user_row.username,
            first_name: user_row.first_name,
            last_name: user_row.last_name,
            phone: user_row.phone,
            is_active: user_row.is_active,
            role: crate::gql::types::Role::from(user_row.role),
        };

        Ok(AuthPayload { token, user })
    }

    /// Enter tournament results (managers only)
    async fn enter_tournament_results(
        &self,
        ctx: &Context<'_>,
        input: EnterTournamentResultsInput,
    ) -> Result<EnterTournamentResultsResponse> {
        use crate::auth::permissions::require_role;

        // Require manager role
        let manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;
        let result_repo = TournamentResultRepo::new(state.db.clone());
        let tournament_repo = TournamentRepo::new(state.db.clone());
        let payout_repo = PayoutTemplateRepo::new(state.db.clone());
        let deal_repo = PlayerDealRepo::new(state.db.clone());

        let tournament_id = Uuid::parse_str(input.tournament_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament ID: {}", e)))?;

        // Verify tournament exists
        let tournament = tournament_repo
            .get(tournament_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        // Calculate payouts
        let total_prize_pool =
            calculate_prize_pool(&tournament, input.player_positions.len() as i32)?;
        let payouts = calculate_payouts(
            &payout_repo,
            input.payout_template_id.as_ref(),
            &input.player_positions,
            total_prize_pool,
            input.deal.as_ref(),
        )
        .await?;

        // Create player deal if specified
        let deal = if let Some(deal_input) = input.deal {
            let manager_id = Uuid::parse_str(manager.id.as_str())
                .map_err(|e| async_graphql::Error::new(format!("Invalid manager ID: {}", e)))?;

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

            let total_deal_amount = calculate_deal_total(&deal_input, &payouts)?;

            let deal_data = CreatePlayerDeal {
                tournament_id,
                deal_type: match deal_input.deal_type {
                    DealType::EvenSplit => "even_split".to_string(),
                    DealType::Icm => "icm".to_string(),
                    DealType::Custom => "custom".to_string(),
                },
                affected_positions: deal_input.affected_positions.clone(),
                custom_payouts,
                total_amount_cents: total_deal_amount,
                notes: deal_input.notes.clone(),
                created_by: manager_id,
            };

            Some(deal_repo.create(deal_data).await?)
        } else {
            None
        };

        // Create tournament results
        let mut results = Vec::new();
        for (position_input, payout_amount) in input.player_positions.iter().zip(payouts.iter()) {
            let user_id = Uuid::parse_str(position_input.user_id.as_str())
                .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;

            let result_data = CreateTournamentResult {
                tournament_id,
                user_id,
                final_position: position_input.final_position,
                prize_cents: *payout_amount,
                notes: None,
            };

            let result_row = result_repo.create(result_data).await?;
            results.push(TournamentResult {
                id: result_row.id.into(),
                tournament_id: result_row.tournament_id.into(),
                user_id: result_row.user_id.into(),
                final_position: result_row.final_position,
                prize_cents: result_row.prize_cents,
                points: result_row.points,
                notes: result_row.notes,
                created_at: result_row.created_at,
            });
        }

        // Convert deal to GraphQL type
        let gql_deal = if let Some(deal_row) = deal {
            let custom_payouts = if let Some(payouts_json) = &deal_row.custom_payouts {
                let payouts_obj = payouts_json
                    .as_object()
                    .ok_or_else(|| async_graphql::Error::new("Invalid custom payouts format"))?;

                let mut custom_payouts_vec = Vec::new();
                for (user_id, amount) in payouts_obj {
                    let amount_cents = amount
                        .as_i64()
                        .ok_or_else(|| async_graphql::Error::new("Invalid payout amount"))?
                        as i32;
                    custom_payouts_vec.push(super::types::CustomPayout {
                        user_id: user_id.clone().into(),
                        amount_cents,
                    });
                }
                Some(custom_payouts_vec)
            } else {
                None
            };

            let deal_type = match deal_row.deal_type.as_str() {
                "even_split" => DealType::EvenSplit,
                "icm" => DealType::Icm,
                "custom" => DealType::Custom,
                _ => DealType::EvenSplit,
            };

            Some(PlayerDeal {
                id: deal_row.id.into(),
                tournament_id: deal_row.tournament_id.into(),
                deal_type,
                affected_positions: deal_row.affected_positions,
                custom_payouts,
                total_amount_cents: deal_row.total_amount_cents,
                notes: deal_row.notes,
                created_by: deal_row.created_by.into(),
            })
        } else {
            None
        };

        Ok(EnterTournamentResultsResponse {
            success: true,
            results,
            deal: gql_deal,
        })
    }

    /// Assign a club table to a tournament (managers only)
    async fn assign_table_to_tournament(
        &self,
        ctx: &Context<'_>,
        input: AssignTableToTournamentInput,
    ) -> Result<TournamentTable> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;
        let tournament_id = Uuid::parse_str(input.tournament_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament ID: {}", e)))?;
        let club_table_id = Uuid::parse_str(input.club_table_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid club table ID: {}", e)))?;

        // Get club ID for the tournament to verify permissions
        let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;

        // Require manager role for this specific club
        let _manager = require_club_manager(ctx, club_id).await?;

        let club_table_repo = ClubTableRepo::new(state.db.clone());

        // Verify the club table belongs to the same club as the tournament
        let club_table = club_table_repo
            .get_by_id(club_table_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Club table not found"))?;

        if club_table.club_id != club_id {
            return Err(async_graphql::Error::new(
                "Club table does not belong to the tournament's club",
            ));
        }

        // Assign the table to the tournament
        let _assignment = club_table_repo
            .assign_to_tournament(tournament_id, club_table_id)
            .await?;

        // Publish seating change event
        let event = SeatingChangeEvent {
            event_type: SeatingEventType::TableCreated,
            tournament_id: tournament_id.into(),
            club_id: club_id.into(),
            affected_assignment: None,
            affected_player: None,
            message: format!("Table {} assigned to tournament", club_table.table_number),
            timestamp: chrono::Utc::now(),
        };
        publish_seating_event(event);

        Ok(TournamentTable {
            id: club_table.id.into(),
            tournament_id: tournament_id.into(),
            table_number: club_table.table_number,
            max_seats: club_table.max_seats,
            is_active: club_table.is_active,
            table_name: club_table.table_name,
            created_at: club_table.created_at,
        })
    }

    /// Assign a player to a specific seat (managers only)
    async fn assign_player_to_seat(
        &self,
        ctx: &Context<'_>,
        input: AssignPlayerToSeatInput,
    ) -> Result<SeatAssignment> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;
        let tournament_id = Uuid::parse_str(input.tournament_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament ID: {}", e)))?;

        // Get club ID for the tournament to verify permissions
        let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;

        // Require manager role for this specific club
        let manager = require_club_manager(ctx, club_id).await?;

        let assignment_repo = TableSeatAssignmentRepo::new(state.db.clone());
        let club_table_id = Uuid::parse_str(input.club_table_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid table ID: {}", e)))?;
        let user_id = Uuid::parse_str(input.user_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;
        let manager_id = Uuid::parse_str(manager.id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid manager ID: {}", e)))?;

        // Check if seat is available
        let is_available = assignment_repo
            .is_seat_available(club_table_id, input.seat_number)
            .await?;
        if !is_available {
            return Err(async_graphql::Error::new("Seat is already occupied"));
        }

        let create_data = CreateSeatAssignment {
            tournament_id,
            club_table_id,
            user_id,
            seat_number: input.seat_number,
            stack_size: input.stack_size,
            assigned_by: Some(manager_id),
            notes: input.notes,
        };

        let assignment_row = assignment_repo.create(create_data).await?;

        // Get player info for the event
        let user_repo = UserRepo::new(state.db.clone());
        let player = user_repo.get_by_id(user_id).await?;

        // Publish seating change event
        let event = SeatingChangeEvent {
            event_type: SeatingEventType::PlayerAssigned,
            tournament_id: assignment_row.tournament_id.into(),
            club_id: club_id.into(),
            affected_assignment: Some(SeatAssignment {
                id: assignment_row.id.into(),
                tournament_id: assignment_row.tournament_id.into(),
                club_table_id: assignment_row.club_table_id.into(),
                user_id: assignment_row.user_id.into(),
                seat_number: assignment_row.seat_number,
                stack_size: assignment_row.stack_size,
                is_current: assignment_row.is_current,
                assigned_at: assignment_row.assigned_at,
                unassigned_at: None, // Field not yet implemented in database
                assigned_by: None,   // Field not yet implemented in database
                notes: None,         // Field not yet implemented in database
            }),
            affected_player: player.map(|p| User {
                id: p.id.into(),
                email: p.email,
                username: p.username,
                first_name: p.first_name,
                last_name: p.last_name,
                phone: p.phone,
                is_active: p.is_active,
                role: crate::gql::types::Role::from(p.role),
            }),
            message: format!("Player assigned to seat {}", assignment_row.seat_number),
            timestamp: chrono::Utc::now(),
        };
        publish_seating_event(event);

        Ok(SeatAssignment {
            id: assignment_row.id.into(),
            tournament_id: assignment_row.tournament_id.into(),
            club_table_id: assignment_row.club_table_id.into(),
            user_id: assignment_row.user_id.into(),
            seat_number: assignment_row.seat_number,
            stack_size: assignment_row.stack_size,
            is_current: assignment_row.is_current,
            assigned_at: assignment_row.assigned_at,
            unassigned_at: None, // Field not yet implemented in database
            assigned_by: None,   // Field not yet implemented in database
            notes: None,         // Field not yet implemented in database
        })
    }

    /// Move a player to a different table/seat (managers only)
    async fn move_player(
        &self,
        ctx: &Context<'_>,
        input: MovePlayerInput,
    ) -> Result<SeatAssignment> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;
        let tournament_id = Uuid::parse_str(input.tournament_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament ID: {}", e)))?;

        // Get club ID for the tournament to verify permissions
        let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;

        // Require manager role for this specific club
        let manager = require_club_manager(ctx, club_id).await?;

        let assignment_repo = TableSeatAssignmentRepo::new(state.db.clone());
        let user_id = Uuid::parse_str(input.user_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;
        let new_club_table_id = Uuid::parse_str(input.new_club_table_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid table ID: {}", e)))?;
        let manager_id = Uuid::parse_str(manager.id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid manager ID: {}", e)))?;

        // Check if new seat is available
        let is_available = assignment_repo
            .is_seat_available(new_club_table_id, input.new_seat_number)
            .await?;
        if !is_available {
            return Err(async_graphql::Error::new("Target seat is already occupied"));
        }

        let assignment_row = assignment_repo
            .move_player(
                tournament_id,
                user_id,
                new_club_table_id,
                input.new_seat_number,
                Some(manager_id),
                input.notes,
            )
            .await?;

        // Get player info for the event
        let user_repo = UserRepo::new(state.db.clone());
        let player = user_repo.get_by_id(user_id).await?;

        // Publish seating change event
        let club_id = get_club_id_for_tournament(&state.db, assignment_row.tournament_id).await?;
        let event = SeatingChangeEvent {
            event_type: SeatingEventType::PlayerMoved,
            tournament_id: assignment_row.tournament_id.into(),
            club_id: club_id.into(),
            affected_assignment: Some(SeatAssignment {
                id: assignment_row.id.into(),
                tournament_id: assignment_row.tournament_id.into(),
                club_table_id: assignment_row.club_table_id.into(),
                user_id: assignment_row.user_id.into(),
                seat_number: assignment_row.seat_number,
                stack_size: assignment_row.stack_size,
                is_current: assignment_row.is_current,
                assigned_at: assignment_row.assigned_at,
                unassigned_at: None, // Field not yet implemented in database
                assigned_by: None,   // Field not yet implemented in database
                notes: None,         // Field not yet implemented in database
            }),
            affected_player: player.map(|p| User {
                id: p.id.into(),
                email: p.email,
                username: p.username,
                first_name: p.first_name,
                last_name: p.last_name,
                phone: p.phone,
                is_active: p.is_active,
                role: crate::gql::types::Role::from(p.role),
            }),
            message: format!("Player moved to seat {}", assignment_row.seat_number),
            timestamp: chrono::Utc::now(),
        };
        publish_seating_event(event);

        Ok(SeatAssignment {
            id: assignment_row.id.into(),
            tournament_id: assignment_row.tournament_id.into(),
            club_table_id: assignment_row.club_table_id.into(),
            user_id: assignment_row.user_id.into(),
            seat_number: assignment_row.seat_number,
            stack_size: assignment_row.stack_size,
            is_current: assignment_row.is_current,
            assigned_at: assignment_row.assigned_at,
            unassigned_at: None, // Field not yet implemented in database
            assigned_by: None,   // Field not yet implemented in database
            notes: None,         // Field not yet implemented in database
        })
    }

    /// Update a player's stack size (managers only)
    async fn update_stack_size(
        &self,
        ctx: &Context<'_>,
        input: UpdateStackSizeInput,
    ) -> Result<SeatAssignment> {
        use crate::auth::permissions::require_role;

        // Require manager role
        let _manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;
        let assignment_repo = TableSeatAssignmentRepo::new(state.db.clone());

        let tournament_id = Uuid::parse_str(input.tournament_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament ID: {}", e)))?;
        let user_id = Uuid::parse_str(input.user_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;

        // Get current assignment for user
        let current_assignment = assignment_repo
            .get_current_for_user(tournament_id, user_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Player not currently assigned to a seat"))?;

        let update_data = UpdateSeatAssignment {
            stack_size: Some(input.new_stack_size),
            notes: None,
        };

        let assignment_row = assignment_repo
            .update(current_assignment.id, update_data)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Failed to update seat assignment"))?;

        // Get player info for the event
        let user_repo = UserRepo::new(state.db.clone());
        let player = user_repo.get_by_id(user_id).await?;

        // Publish seating change event
        let club_id = get_club_id_for_tournament(&state.db, assignment_row.tournament_id).await?;
        let event = SeatingChangeEvent {
            event_type: SeatingEventType::StackUpdated,
            tournament_id: assignment_row.tournament_id.into(),
            club_id: club_id.into(),
            affected_assignment: Some(SeatAssignment {
                id: assignment_row.id.into(),
                tournament_id: assignment_row.tournament_id.into(),
                club_table_id: assignment_row.club_table_id.into(),
                user_id: assignment_row.user_id.into(),
                seat_number: assignment_row.seat_number,
                stack_size: assignment_row.stack_size,
                is_current: assignment_row.is_current,
                assigned_at: assignment_row.assigned_at,
                unassigned_at: None, // Field not yet implemented in database
                assigned_by: None,   // Field not yet implemented in database
                notes: None,         // Field not yet implemented in database
            }),
            affected_player: player.map(|p| User {
                id: p.id.into(),
                email: p.email,
                username: p.username,
                first_name: p.first_name,
                last_name: p.last_name,
                phone: p.phone,
                is_active: p.is_active,
                role: crate::gql::types::Role::from(p.role),
            }),
            message: format!("Stack updated to {}", input.new_stack_size),
            timestamp: chrono::Utc::now(),
        };
        publish_seating_event(event);

        Ok(SeatAssignment {
            id: assignment_row.id.into(),
            tournament_id: assignment_row.tournament_id.into(),
            club_table_id: assignment_row.club_table_id.into(),
            user_id: assignment_row.user_id.into(),
            seat_number: assignment_row.seat_number,
            stack_size: assignment_row.stack_size,
            is_current: assignment_row.is_current,
            assigned_at: assignment_row.assigned_at,
            unassigned_at: None, // Field not yet implemented in database
            assigned_by: None,   // Field not yet implemented in database
            notes: None,         // Field not yet implemented in database
        })
    }

    /// Update tournament live status (managers only)
    async fn update_tournament_status(
        &self,
        ctx: &Context<'_>,
        input: UpdateTournamentStatusInput,
    ) -> Result<Tournament> {
        use crate::auth::permissions::require_role;

        // Require manager role
        let _manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;
        let tournament_repo = TournamentRepo::new(state.db.clone());

        let tournament_id = Uuid::parse_str(input.tournament_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament ID: {}", e)))?;

        let live_status = match input.live_status {
            crate::gql::types::TournamentLiveStatus::NotStarted => TournamentLiveStatus::NotStarted,
            crate::gql::types::TournamentLiveStatus::RegistrationOpen => {
                TournamentLiveStatus::RegistrationOpen
            }
            crate::gql::types::TournamentLiveStatus::LateRegistration => {
                TournamentLiveStatus::LateRegistration
            }
            crate::gql::types::TournamentLiveStatus::InProgress => TournamentLiveStatus::InProgress,
            crate::gql::types::TournamentLiveStatus::Break => TournamentLiveStatus::Break,
            crate::gql::types::TournamentLiveStatus::FinalTable => TournamentLiveStatus::FinalTable,
            crate::gql::types::TournamentLiveStatus::Finished => TournamentLiveStatus::Finished,
        };

        let tournament_row = tournament_repo
            .update_live_status(tournament_id, live_status)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        // Publish seating change event
        let event = SeatingChangeEvent {
            event_type: SeatingEventType::TournamentStatusChanged,
            tournament_id: tournament_row.id.into(),
            club_id: tournament_row.club_id.into(),
            affected_assignment: None,
            affected_player: None,
            message: format!("Tournament status changed to {:?}", input.live_status),
            timestamp: chrono::Utc::now(),
        };
        publish_seating_event(event);

        Ok(Tournament {
            id: tournament_row.id.into(),
            title: tournament_row.name.clone(),
            description: tournament_row.description.clone(),
            club_id: tournament_row.club_id.into(),
            start_time: tournament_row.start_time,
            end_time: tournament_row.end_time,
            buy_in_cents: tournament_row.buy_in_cents,
            seat_cap: tournament_row.seat_cap,
            status: tournament_row.calculate_status().into(),
            live_status: tournament_row.live_status.into(),
            created_at: tournament_row.created_at,
            updated_at: tournament_row.updated_at,
        })
    }

    /// Update tournament state (live data like current level, blinds, etc.) - managers only
    async fn update_tournament_state(
        &self,
        ctx: &Context<'_>,
        input: UpdateTournamentStateInput,
    ) -> Result<TournamentState> {
        use crate::auth::permissions::require_role;

        // Require manager role
        let _manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;
        let tournament_repo = TournamentRepo::new(state.db.clone());

        let tournament_id = Uuid::parse_str(input.tournament_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament ID: {}", e)))?;

        let update_data = UpdateTournamentState {
            current_level: input.current_level,
            players_remaining: input.players_remaining,
            break_until: input.break_until,
            current_small_blind: input.current_small_blind,
            current_big_blind: input.current_big_blind,
            current_ante: input.current_ante,
            level_started_at: input.level_started_at,
            level_duration_minutes: input.level_duration_minutes,
        };

        let state_row = tournament_repo
            .upsert_state(tournament_id, update_data)
            .await?;

        Ok(TournamentState {
            id: state_row.id.into(),
            tournament_id: state_row.tournament_id.into(),
            current_level: state_row.current_level,
            players_remaining: state_row.players_remaining,
            break_until: state_row.break_until,
            current_small_blind: state_row.current_small_blind,
            current_big_blind: state_row.current_big_blind,
            current_ante: state_row.current_ante,
            level_started_at: state_row.level_started_at,
            level_duration_minutes: state_row.level_duration_minutes,
            created_at: state_row.created_at,
            updated_at: state_row.updated_at,
        })
    }

    /// Automatically balance tables (managers only)
    async fn balance_tables(
        &self,
        ctx: &Context<'_>,
        input: BalanceTablesInput,
    ) -> Result<Vec<SeatAssignment>> {
        use crate::auth::permissions::require_role;

        // Require manager role
        let manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;
        let club_table_repo = ClubTableRepo::new(state.db.clone());
        let assignment_repo = TableSeatAssignmentRepo::new(state.db.clone());

        let tournament_id = Uuid::parse_str(input.tournament_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament ID: {}", e)))?;
        let manager_id = Uuid::parse_str(manager.id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid manager ID: {}", e)))?;

        // Get all active tables
        let tables = club_table_repo
            .get_assigned_to_tournament(tournament_id)
            .await?;
        if tables.is_empty() {
            return Ok(Vec::new());
        }

        // Get all current assignments
        let assignments = assignment_repo
            .get_current_for_tournament(tournament_id)
            .await?;

        // Group players by table
        let mut table_players: std::collections::HashMap<uuid::Uuid, Vec<_>> =
            std::collections::HashMap::new();
        for assignment in assignments {
            table_players
                .entry(assignment.club_table_id)
                .or_default()
                .push(assignment);
        }

        // Calculate target players per table
        let total_players = table_players.values().map(|v| v.len()).sum::<usize>();
        let target_per_table = input
            .target_players_per_table
            .unwrap_or(((total_players as f64) / (tables.len() as f64)).ceil() as i32);

        // Simple balancing: move excess players from over-populated tables to under-populated ones
        let mut moves = Vec::new();

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
                // Take the most recently assigned players for moving (they're likely less settled)
                let mut sorted_players = players.clone();
                sorted_players.sort_by(|a, b| b.assigned_at.cmp(&a.assigned_at));
                excess_players.extend(sorted_players.into_iter().take(excess_count));
            }
        }

        // Move excess players to tables that need them
        for player in excess_players {
            if let Some(target_table) = need_players.first() {
                let current_count = table_players
                    .get(&target_table.id)
                    .map(|v| v.len())
                    .unwrap_or(0);
                if current_count < target_per_table as usize {
                    // Find an available seat
                    for seat_num in 1..=target_table.max_seats {
                        let is_available = assignment_repo
                            .is_seat_available(target_table.id, seat_num)
                            .await?;
                        if is_available {
                            // Move the player
                            let new_assignment = assignment_repo
                                .move_player(
                                    tournament_id,
                                    player.user_id,
                                    target_table.id,
                                    seat_num,
                                    Some(manager_id),
                                    Some("Balanced by system".to_string()),
                                )
                                .await?;

                            let assignment_for_response = SeatAssignment {
                                id: new_assignment.id.into(),
                                tournament_id: new_assignment.tournament_id.into(),
                                club_table_id: new_assignment.club_table_id.into(),
                                user_id: new_assignment.user_id.into(),
                                seat_number: new_assignment.seat_number,
                                stack_size: new_assignment.stack_size,
                                is_current: new_assignment.is_current,
                                assigned_at: new_assignment.assigned_at,
                                unassigned_at: None, // Field not yet implemented in database
                                assigned_by: None,   // Field not yet implemented in database
                                notes: None,         // Field not yet implemented in database
                            };

                            moves.push(assignment_for_response);

                            // Update our tracking
                            table_players
                                .entry(target_table.id)
                                .or_default()
                                .push(new_assignment);

                            // Check if this table is now full
                            if table_players.get(&target_table.id).unwrap().len()
                                >= target_per_table as usize
                            {
                                need_players.remove(0);
                            }
                            break;
                        }
                    }
                }
            }
        }

        // Publish table balancing event if moves were made
        if !moves.is_empty() {
            let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;
            let event = SeatingChangeEvent {
                event_type: SeatingEventType::TablesBalanced,
                tournament_id: tournament_id.into(),
                club_id: club_id.into(),
                affected_assignment: None,
                affected_player: None,
                message: format!("{} players moved to balance tables", moves.len()),
                timestamp: chrono::Utc::now(),
            };
            publish_seating_event(event);
        }

        Ok(moves)
    }

    /// Eliminate a player from the tournament (managers only)
    async fn eliminate_player(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
        user_id: ID,
        notes: Option<String>,
    ) -> Result<bool> {
        use crate::auth::permissions::require_role;

        // Require manager role
        let manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;
        let assignment_repo = TableSeatAssignmentRepo::new(state.db.clone());

        let tournament_uuid = Uuid::parse_str(tournament_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament ID: {}", e)))?;
        let user_uuid = Uuid::parse_str(user_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;
        let manager_id = Uuid::parse_str(manager.id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid manager ID: {}", e)))?;

        // Get current assignment for user
        let current_assignment = assignment_repo
            .get_current_for_user(tournament_uuid, user_uuid)
            .await?;

        if let Some(assignment) = current_assignment {
            // Update the assignment with elimination notes and unassign
            let update_data = UpdateSeatAssignment {
                stack_size: Some(0), // Set stack to 0 to indicate elimination
                notes: notes
                    .clone()
                    .or_else(|| Some("Player eliminated".to_string())),
            };

            assignment_repo.update(assignment.id, update_data).await?;
            assignment_repo
                .unassign(assignment.id, Some(manager_id))
                .await?;

            // Get player info for the event
            let user_repo = UserRepo::new(state.db.clone());
            let player = user_repo.get_by_id(user_uuid).await?;

            // Publish seating change event
            let club_id = get_club_id_for_tournament(&state.db, tournament_uuid).await?;
            let event = SeatingChangeEvent {
                event_type: SeatingEventType::PlayerEliminated,
                tournament_id: tournament_uuid.into(),
                club_id: club_id.into(),
                affected_assignment: Some(SeatAssignment {
                    id: assignment.id.into(),
                    tournament_id: assignment.tournament_id.into(),
                    club_table_id: assignment.club_table_id.into(),
                    user_id: assignment.user_id.into(),
                    seat_number: assignment.seat_number,
                    stack_size: Some(0),
                    is_current: false,
                    assigned_at: assignment.assigned_at,
                    unassigned_at: Some(chrono::Utc::now()),
                    assigned_by: None, // Field not yet implemented in database
                    notes: notes.or_else(|| Some("Player eliminated".to_string())),
                }),
                affected_player: player.map(|p| User {
                    id: p.id.into(),
                    email: p.email,
                    username: p.username,
                    first_name: p.first_name,
                    last_name: p.last_name,
                    phone: p.phone,
                    is_active: p.is_active,
                    role: crate::gql::types::Role::from(p.role),
                }),
                message: "Player eliminated from tournament".to_string(),
                timestamp: chrono::Utc::now(),
            };
            publish_seating_event(event);

            Ok(true)
        } else {
            Err(async_graphql::Error::new(
                "Player not currently assigned to a seat",
            ))
        }
    }
}

fn generate_client_id() -> String {
    format!(
        "client_{}",
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect::<String>()
    )
}

fn generate_client_secret() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

async fn find_user_by_email(state: &AppState, email: &str) -> Result<Option<User>> {
    let row = sqlx::query!(
        "SELECT id, email, username, first_name, last_name, phone, is_active, role FROM users WHERE email = $1",
        email
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| async_graphql::Error::new(e.to_string()))?;

    match row {
        Some(row) => Ok(Some(User {
            id: row.id.into(),
            email: row.email,
            username: row.username,
            first_name: row.first_name,
            last_name: row.last_name,
            phone: row.phone,
            is_active: row.is_active,
            role: crate::gql::types::Role::from(row.role),
        })),
        None => Ok(None),
    }
}

async fn create_user_from_oauth(
    state: &AppState,
    oauth_user: &crate::auth::oauth::OAuthUserInfo,
    provider: &str,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO users (email, username, first_name, last_name, oauth_provider, oauth_provider_id, avatar_url)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#,
        oauth_user.email,
        oauth_user.username,
        oauth_user.first_name,
        oauth_user.last_name,
        provider,
        oauth_user.provider_id,
        oauth_user.avatar_url
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| async_graphql::Error::new(e.to_string()))?;

    Ok(row.id)
}

async fn get_user_by_id(state: &AppState, user_id: Uuid) -> Result<User> {
    let row = sqlx::query!(
        "SELECT id, email, username, first_name, last_name, phone, is_active, role FROM users WHERE id = $1",
        user_id
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| async_graphql::Error::new(e.to_string()))?;

    Ok(User {
        id: row.id.into(),
        email: row.email,
        username: row.username,
        first_name: row.first_name,
        last_name: row.last_name,
        phone: row.phone,
        is_active: row.is_active,
        role: crate::gql::types::Role::from(row.role),
    })
}

// Payout calculation functions

fn calculate_prize_pool(tournament: &TournamentRow, player_count: i32) -> Result<i32> {
    // Calculate total prize pool based on buy-in and player count
    let total_pool = tournament.buy_in_cents * player_count;
    Ok(total_pool)
}

async fn calculate_payouts(
    payout_repo: &PayoutTemplateRepo,
    template_id: Option<&ID>,
    positions: &[PlayerPositionInput],
    total_prize_pool: i32,
    deal: Option<&PlayerDealInput>,
) -> Result<Vec<i32>> {
    let mut payouts = vec![0; positions.len()];

    // If there's a deal that affects certain positions, handle it
    if let Some(deal_input) = deal {
        match deal_input.deal_type {
            DealType::EvenSplit => {
                // Calculate total amount for affected positions using template
                let affected_total = if let Some(template_id) = template_id {
                    calculate_template_total(
                        payout_repo,
                        template_id,
                        &deal_input.affected_positions,
                        total_prize_pool,
                    )
                    .await?
                } else {
                    // Default: affected positions get equal share of remaining pool
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
                // Use custom payouts if provided
                if let Some(custom_payouts) = &deal_input.custom_payouts {
                    for position in positions {
                        if deal_input
                            .affected_positions
                            .contains(&position.final_position)
                        {
                            // Find custom payout for this user
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
            DealType::Icm => {
                // ICM calculation would be more complex - for now, fall back to even split
                let affected_total = if let Some(template_id) = template_id {
                    calculate_template_total(
                        payout_repo,
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

        // Calculate remaining positions using template if available
        if let Some(template_id) = template_id {
            let template_id_uuid = Uuid::parse_str(template_id.as_str())
                .map_err(|e| async_graphql::Error::new(format!("Invalid template ID: {}", e)))?;

            if let Some(template) = payout_repo.get_by_id(template_id_uuid).await? {
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
                                    (total_prize_pool as f64 * percentage / 100.0) as i32;
                            }
                        }
                    }
                }
            }
        }
    } else {
        // No deal - use template for all positions
        if let Some(template_id) = template_id {
            let template_id_uuid = Uuid::parse_str(template_id.as_str())
                .map_err(|e| async_graphql::Error::new(format!("Invalid template ID: {}", e)))?;

            if let Some(template) = payout_repo.get_by_id(template_id_uuid).await? {
                let payout_structure = parse_payout_structure(&template.payout_structure)?;

                for position in positions {
                    if let Some(percentage) =
                        get_position_percentage(&payout_structure, position.final_position)
                    {
                        let index = (position.final_position - 1) as usize;
                        if index < payouts.len() {
                            payouts[index] = (total_prize_pool as f64 * percentage / 100.0) as i32;
                        }
                    }
                }
            }
        }
    }

    Ok(payouts)
}

async fn calculate_template_total(
    payout_repo: &PayoutTemplateRepo,
    template_id: &ID,
    affected_positions: &[i32],
    total_prize_pool: i32,
) -> Result<i32> {
    let template_id_uuid = Uuid::parse_str(template_id.as_str())
        .map_err(|e| async_graphql::Error::new(format!("Invalid template ID: {}", e)))?;

    if let Some(template) = payout_repo.get_by_id(template_id_uuid).await? {
        let payout_structure = parse_payout_structure(&template.payout_structure)?;
        let mut total_percentage = 0.0;

        for position in affected_positions {
            if let Some(percentage) = get_position_percentage(&payout_structure, *position) {
                total_percentage += percentage;
            }
        }

        Ok((total_prize_pool as f64 * total_percentage / 100.0) as i32)
    } else {
        Ok(total_prize_pool)
    }
}

fn calculate_deal_total(deal_input: &PlayerDealInput, payouts: &[i32]) -> Result<i32> {
    match deal_input.deal_type {
        DealType::Custom => {
            if let Some(custom_payouts) = &deal_input.custom_payouts {
                Ok(custom_payouts.iter().map(|p| p.amount_cents).sum())
            } else {
                Ok(0)
            }
        }
        _ => {
            // For even split and ICM, calculate from affected positions
            let mut total = 0;
            for position in &deal_input.affected_positions {
                let index = (*position - 1) as usize;
                if index < payouts.len() {
                    total += payouts[index];
                }
            }
            Ok(total)
        }
    }
}

fn parse_payout_structure(structure: &serde_json::Value) -> Result<Vec<(i32, f64)>> {
    let array = structure
        .as_array()
        .ok_or_else(|| async_graphql::Error::new("Invalid payout structure format"))?;

    let mut payouts = Vec::new();
    for item in array {
        let obj = item
            .as_object()
            .ok_or_else(|| async_graphql::Error::new("Invalid payout item format"))?;

        let position = obj
            .get("position")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| async_graphql::Error::new("Missing or invalid position"))?
            as i32;

        let percentage = obj
            .get("percentage")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| async_graphql::Error::new("Missing or invalid percentage"))?;

        payouts.push((position, percentage));
    }

    Ok(payouts)
}

fn get_position_percentage(payout_structure: &[(i32, f64)], position: i32) -> Option<f64> {
    payout_structure
        .iter()
        .find(|(pos, _)| *pos == position)
        .map(|(_, percentage)| *percentage)
}
