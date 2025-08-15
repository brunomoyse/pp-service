use async_graphql::{Context, InputObject, Object, Result, ID};

use crate::auth::{Claims, OAuthProvider, password::PasswordService, custom_oauth::CustomOAuthService, permissions::require_admin_if};
use crate::state::AppState;
use super::types::{Tournament, TournamentRegistration, RegisterForTournamentInput, TournamentPlayer, User, PlayerRegistrationEvent, AuthPayload, OAuthUrlResponse, OAuthCallbackInput, OAuthClient, CreateOAuthClientInput, CreateOAuthClientResponse, UserRegistrationInput, UserLoginInput};
use super::subscriptions::publish_registration_event;
use infra::repos::{TournamentRegistrationRepo, CreateTournamentRegistration, UserRepo};
use uuid::Uuid;
use rand::{distributions::Alphanumeric, Rng};

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
                role: user_row.role.unwrap_or_else(|| "user".to_string()),
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
            role: "user".to_string(),
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
            role: user_row.role,
        };

        Ok(AuthPayload { token, user })
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
            role: row.role,
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
        role: row.role,
    })
}

