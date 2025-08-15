use async_graphql::{Context, InputObject, Object, Result, ID};

use crate::auth::{Claims, OAuthProvider, password::PasswordService, custom_oauth::CustomOAuthService, permissions::require_admin_if};
use crate::state::AppState;
use super::types::{Tournament, TournamentRegistration, RegisterForTournamentInput, TournamentPlayer, User, PlayerRegistrationEvent, AuthPayload, OAuthUrlResponse, OAuthCallbackInput, OAuthClient, CreateOAuthClientInput, CreateOAuthClientResponse, UserRegistrationInput, UserLoginInput, EnterTournamentResultsInput, EnterTournamentResultsResponse, TournamentResult, PlayerDeal, DealType, Role, PlayerPositionInput, PlayerDealInput};
use super::subscriptions::publish_registration_event;
use infra::repos::{TournamentRegistrationRepo, CreateTournamentRegistration, UserRepo, TournamentResultRepo, CreateTournamentResult, PayoutTemplateRepo, PlayerDealRepo, CreatePlayerDeal, TournamentRepo};
use infra::models::TournamentRow;
use uuid::Uuid;
use rand::{distributions::Alphanumeric, Rng};
use serde_json;

pub struct MutationRoot;

#[derive(InputObject)]
pub struct CreateTournamentInput {
    pub title: String,
    pub club_id: ID,
}

#[Object]
impl MutationRoot {
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
            club_id: input.club_id,
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
        let authenticated_user = require_admin_if(ctx, is_admin_registration, "user_id").await?
            .ok_or_else(|| async_graphql::Error::new("Authentication required"))?;
        
        // Determine which user to register
        let user_id = match input.user_id {
            Some(target_user_id) => {
                // Admin is registering another user
                Uuid::parse_str(target_user_id.as_str())
                    .map_err(|e| async_graphql::Error::new(format!("Invalid target user ID: {}", e)))?
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
    async fn get_oauth_url(
        &self,
        ctx: &Context<'_>,
        provider: String,
    ) -> Result<OAuthUrlResponse> {
        let state = ctx.data::<AppState>()?;
        
        let provider = match provider.as_str() {
            "google" => OAuthProvider::Google,
            "custom" => OAuthProvider::Custom,
            _ => return Err(async_graphql::Error::new("Invalid OAuth provider")),
        };

        let (auth_url, csrf_token) = state.oauth_service().get_authorize_url(provider)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(OAuthUrlResponse { auth_url, csrf_token })
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
            _ => {
                state
                    .oauth_service()
                    .exchange_code_for_user_info(provider, input.code)
                    .await
                    .map_err(|e| async_graphql::Error::new(e.to_string()))?
            }
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
        let claims = ctx.data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;

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
    async fn register_user(
        &self,
        ctx: &Context<'_>,
        input: UserRegistrationInput,
    ) -> Result<User> {
        let state = ctx.data::<AppState>()?;
        
        // Validate password strength
        PasswordService::validate_password_strength(&input.password)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Hash password
        let password_hash = PasswordService::hash_password(&input.password)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Check if user already exists
        let existing_user = sqlx::query!(
            "SELECT id FROM users WHERE email = $1",
            input.email
        )
        .fetch_optional(&state.db)
        .await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        if existing_user.is_some() {
            return Err(async_graphql::Error::new("User with this email already exists"));
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
    async fn login_user(
        &self,
        ctx: &Context<'_>,
        input: UserLoginInput,
    ) -> Result<AuthPayload> {
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
            if !PasswordService::verify_password(&input.password, &password_hash)
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
        let tournament = tournament_repo.get(tournament_id).await?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;
        
        // Calculate payouts
        let total_prize_pool = calculate_prize_pool(&tournament, input.player_positions.len() as i32)?;
        let payouts = calculate_payouts(
            &payout_repo,
            input.payout_template_id.as_ref(),
            &input.player_positions,
            total_prize_pool,
            input.deal.as_ref(),
        ).await?;
        
        // Create player deal if specified
        let deal = if let Some(deal_input) = input.deal {
            let manager_id = Uuid::parse_str(manager.id.as_str())
                .map_err(|e| async_graphql::Error::new(format!("Invalid manager ID: {}", e)))?;
            
            let custom_payouts = if let Some(custom) = &deal_input.custom_payouts {
                let mut payouts_map = serde_json::Map::new();
                for payout in custom {
                    payouts_map.insert(
                        payout.user_id.to_string(),
                        serde_json::Value::Number(serde_json::Number::from(payout.amount_cents))
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
                notes: result_row.notes,
                created_at: result_row.created_at,
            });
        }
        
        // Convert deal to GraphQL type
        let gql_deal = if let Some(deal_row) = deal {
            let custom_payouts = if let Some(payouts_json) = &deal_row.custom_payouts {
                let payouts_obj = payouts_json.as_object()
                    .ok_or_else(|| async_graphql::Error::new("Invalid custom payouts format"))?;
                
                let mut custom_payouts_vec = Vec::new();
                for (user_id, amount) in payouts_obj {
                    let amount_cents = amount.as_i64()
                        .ok_or_else(|| async_graphql::Error::new("Invalid payout amount"))? as i32;
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
}

fn generate_client_id() -> String {
    format!("client_{}", rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect::<String>())
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
                    calculate_template_total(payout_repo, template_id, &deal_input.affected_positions, total_prize_pool).await?
                } else {
                    // Default: affected positions get equal share of remaining pool
                    total_prize_pool
                };
                
                let per_player = affected_total / deal_input.affected_positions.len() as i32;
                
                for position in positions {
                    if deal_input.affected_positions.contains(&position.final_position) {
                        let index = (position.final_position - 1) as usize;
                        if index < payouts.len() {
                            payouts[index] = per_player;
                        }
                    }
                }
            },
            DealType::Custom => {
                // Use custom payouts if provided
                if let Some(custom_payouts) = &deal_input.custom_payouts {
                    for position in positions {
                        if deal_input.affected_positions.contains(&position.final_position) {
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
            },
            DealType::Icm => {
                // ICM calculation would be more complex - for now, fall back to even split
                let affected_total = if let Some(template_id) = template_id {
                    calculate_template_total(payout_repo, template_id, &deal_input.affected_positions, total_prize_pool).await?
                } else {
                    total_prize_pool
                };
                
                let per_player = affected_total / deal_input.affected_positions.len() as i32;
                
                for position in positions {
                    if deal_input.affected_positions.contains(&position.final_position) {
                        let index = (position.final_position - 1) as usize;
                        if index < payouts.len() {
                            payouts[index] = per_player;
                        }
                    }
                }
            },
        }
        
        // Calculate remaining positions using template if available
        if let Some(template_id) = template_id {
            let template_id_uuid = Uuid::parse_str(template_id.as_str())
                .map_err(|e| async_graphql::Error::new(format!("Invalid template ID: {}", e)))?;
            
            if let Some(template) = payout_repo.get_by_id(template_id_uuid).await? {
                let payout_structure = parse_payout_structure(&template.payout_structure)?;
                
                for position in positions {
                    if !deal_input.affected_positions.contains(&position.final_position) {
                        if let Some(percentage) = get_position_percentage(&payout_structure, position.final_position) {
                            let index = (position.final_position - 1) as usize;
                            if index < payouts.len() {
                                payouts[index] = (total_prize_pool as f64 * percentage / 100.0) as i32;
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
                    if let Some(percentage) = get_position_percentage(&payout_structure, position.final_position) {
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
        },
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
    let array = structure.as_array()
        .ok_or_else(|| async_graphql::Error::new("Invalid payout structure format"))?;
    
    let mut payouts = Vec::new();
    for item in array {
        let obj = item.as_object()
            .ok_or_else(|| async_graphql::Error::new("Invalid payout item format"))?;
        
        let position = obj.get("position")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| async_graphql::Error::new("Missing or invalid position"))? as i32;
        
        let percentage = obj.get("percentage")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| async_graphql::Error::new("Missing or invalid percentage"))?;
        
        payouts.push((position, percentage));
    }
    
    Ok(payouts)
}

fn get_position_percentage(payout_structure: &[(i32, f64)], position: i32) -> Option<f64> {
    payout_structure.iter()
        .find(|(pos, _)| *pos == position)
        .map(|(_, percentage)| *percentage)
}

