use api::AppState;
use async_graphql::{Request, Variables};
use sqlx::postgres::PgPoolOptions;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::SyncRunner;
use testcontainers_modules::testcontainers::Container;
use testcontainers_modules::testcontainers::ImageExt;
use uuid::Uuid;

struct TestContainer {
    _container: Container<Postgres>,
    db_url: String,
}

// Safety: Container<Postgres> holds a Docker client with Arc internals.
// We never mutate TestContainer after init, and db_url is a plain String.
unsafe impl Send for TestContainer {}
unsafe impl Sync for TestContainer {}

static TEST_CONTAINER: std::sync::OnceLock<TestContainer> = std::sync::OnceLock::new();

fn get_db_url() -> &'static str {
    let tc = TEST_CONTAINER.get_or_init(|| {
        // Spawn a dedicated thread so SyncRunner doesn't conflict with tokio runtime
        std::thread::spawn(|| {
            let container = Postgres::default()
                .with_tag("16-alpine")
                .start()
                .expect("Failed to start Postgres container");

            let host_port = container.get_host_port_ipv4(5432).unwrap();
            let db_url = format!(
                "postgres://postgres:postgres@localhost:{}/postgres",
                host_port
            );

            TestContainer {
                _container: container,
                db_url,
            }
        })
        .join()
        .expect("Container init thread panicked")
    });
    &tc.db_url
}

pub async fn setup_test_db() -> AppState {
    // Each test gets its own pool connected to the shared container.
    // We don't share the pool across tests because each #[tokio::test]
    // creates a separate runtime, and SQLx pool background tasks are
    // tied to the runtime that created them.
    let url = get_db_url();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect(url)
        .await
        .expect("Failed to connect to test database");

    // Run migrations (idempotent — safe to call from every test)
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    AppState::new(pool).expect("Failed to create AppState")
}

/// Helper function to execute GraphQL queries and mutations
#[allow(dead_code)]
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
        role: role.to_string(),
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

/// Create test club table and return its ID
#[allow(dead_code)]
pub async fn create_test_club_table(
    app_state: &AppState,
    club_id: Uuid,
    table_number: i32,
    max_seats: i32,
) -> Uuid {
    let table_id = Uuid::new_v4();

    sqlx::query!(
        "INSERT INTO club_tables (id, club_id, table_number, max_seats) VALUES ($1, $2, $3, $4) ON CONFLICT (id) DO NOTHING",
        table_id,
        club_id,
        table_number,
        max_seats
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test club table");

    table_id
}

/// Create a tournament registration for a user and return its ID
#[allow(dead_code)]
pub async fn create_test_registration(
    app_state: &AppState,
    tournament_id: Uuid,
    user_id: Uuid,
    status: &str,
) -> Uuid {
    let reg_id = Uuid::new_v4();

    sqlx::query!(
        r#"INSERT INTO tournament_registrations (id, tournament_id, user_id, status)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT DO NOTHING"#,
        reg_id,
        tournament_id,
        user_id,
        status
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test registration");

    reg_id
}

/// Assign a club table to a tournament (creates the tournament_table_assignments row)
#[allow(dead_code)]
pub async fn assign_table_to_tournament(
    app_state: &AppState,
    tournament_id: Uuid,
    club_table_id: Uuid,
) {
    sqlx::query!(
        "INSERT INTO tournament_table_assignments (tournament_id, club_table_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        tournament_id,
        club_table_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to assign table to tournament");
}
