use std::env;

use api::AppState;
use async_graphql::{Request, Variables};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

pub async fn setup_test_db() -> AppState {
    let database_url = env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/pocketpair".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    AppState::new(pool).expect("Failed to create AppState")
}

/// Helper function to execute GraphQL queries and mutations
pub async fn execute_graphql(
    schema: &async_graphql::Schema<
        api::gql::QueryRoot,
        api::gql::MutationRoot,
        api::gql::SubscriptionRoot,
    >,
    query: &str,
    variables: Option<Variables>,
    auth_claims: Option<api::auth::Claims>,
) -> async_graphql::Response {
    let mut request = Request::new(query);

    if let Some(vars) = variables {
        request = request.variables(vars);
    }

    if let Some(claims) = auth_claims {
        request = request.data(claims);
    }

    schema.execute(request).await
}

/// Create test user and return JWT claims for authentication
#[allow(dead_code)]
pub async fn create_test_user(
    app_state: &AppState,
    email: &str,
    role: &str,
) -> (Uuid, api::auth::Claims) {
    let user_id = Uuid::new_v4();

    // Insert test user directly into database
    sqlx::query!(
        "INSERT INTO users (id, email, username, first_name, last_name, password_hash, role, is_active) 
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) 
         ON CONFLICT (email) DO UPDATE SET role = $7",
        user_id,
        email,
        format!("test_{}", user_id),
        "Test",
        "User",
        "$2b$12$dummy.hash.for.testing",
        role,
        true
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test user");

    // Get the actual user ID (in case of conflict, it might be different)
    let actual_user = sqlx::query!("SELECT id FROM users WHERE email = $1", email)
        .fetch_one(&app_state.db)
        .await
        .expect("Failed to fetch created user");

    let actual_user_id = actual_user.id;

    let claims = api::auth::Claims {
        sub: actual_user_id.to_string(),
        email: email.to_string(),
        iat: chrono::Utc::now().timestamp(),
        exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp(),
    };

    (actual_user_id, claims)
}

/// Create test club and return its ID
#[allow(dead_code)]
pub async fn create_test_club(app_state: &AppState, name: &str) -> Uuid {
    let club_id = Uuid::new_v4();

    sqlx::query!(
        "INSERT INTO clubs (id, name, city) VALUES ($1, $2, $3) ON CONFLICT (id) DO NOTHING",
        club_id,
        name,
        "Test City"
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test club");

    club_id
}

/// Create test tournament and return its ID
#[allow(dead_code)]
pub async fn create_test_tournament(app_state: &AppState, club_id: Uuid, title: &str) -> Uuid {
    let tournament_id = Uuid::new_v4();

    sqlx::query!(
        r#"INSERT INTO tournaments (
            id, name, description, club_id, start_time, end_time, 
            buy_in_cents, seat_cap, live_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'not_started'::tournament_live_status) 
        ON CONFLICT (id) DO NOTHING"#,
        tournament_id,
        title,
        "Test tournament description",
        club_id,
        chrono::Utc::now(),
        chrono::Utc::now() + chrono::Duration::hours(4),
        5000i32, // $50.00
        100i32
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test tournament");

    tournament_id
}

/// Create club manager relationship
#[allow(dead_code)]
pub async fn create_club_manager(app_state: &AppState, manager_id: Uuid, club_id: Uuid) {
    sqlx::query!(
        "INSERT INTO club_managers (user_id, club_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        manager_id,
        club_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create club manager relationship");
}
