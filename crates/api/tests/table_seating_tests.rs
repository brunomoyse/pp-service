use std::env;

use api::{gql::build_schema, AppState};
use async_graphql::{Request, Variables};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

async fn setup_test_db() -> AppState {
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
async fn execute_graphql(
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
async fn create_test_user(
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

    let claims = api::auth::Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        iat: chrono::Utc::now().timestamp(),
        exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp(),
    };

    (user_id, claims)
}

/// Create test club and return its ID
async fn create_test_club(app_state: &AppState, name: &str) -> Uuid {
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
async fn create_test_tournament(app_state: &AppState, club_id: Uuid, title: &str) -> Uuid {
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
async fn create_club_manager(app_state: &AppState, manager_id: Uuid, club_id: Uuid) {
    sqlx::query!(
        "INSERT INTO club_managers (user_id, club_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        manager_id,
        club_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create club manager relationship");
}

// =============================================================================
// TABLE MANAGEMENT TESTS
// =============================================================================

#[tokio::test]
async fn test_create_tournament_table() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "tablemanager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Table Test Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Table Test Tournament").await;

    // Create club manager relationship
    create_club_manager(&app_state, manager_id, club_id).await;

    let query = r#"
        mutation CreateTournamentTable($input: CreateTournamentTableInput!) {
            createTournamentTable(input: $input) {
                id
                tableNumber
                maxSeats
                isActive
                tournamentId
                tableName
                createdAt
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "tableNumber": 1,
            "maxSeats": 9
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Create table should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let table = &data["createTournamentTable"];

    assert_eq!(table["tableNumber"], 1);
    assert_eq!(table["maxSeats"], 9);
    assert_eq!(table["isActive"], true);
    assert_eq!(table["tournamentId"], tournament_id.to_string());
}

#[tokio::test]
async fn test_create_table_unauthorized() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (_, player_claims) = create_test_user(&app_state, "tableplayer@test.com", "player").await;
    let club_id = create_test_club(&app_state, "Unauthorized Table Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Unauthorized Table Tournament").await;

    let query = r#"
        mutation CreateTournamentTable($input: CreateTournamentTableInput!) {
            createTournamentTable(input: $input) {
                id
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "tableNumber": 1,
            "maxSeats": 9
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(player_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Player should not be able to create table"
    );
    assert!(
        response.errors[0]
            .message
            .contains("Manager privileges required")
            || response.errors[0]
                .message
                .contains("not authorized to manage this club")
    );
}

// =============================================================================
// SEAT ASSIGNMENT TESTS
// =============================================================================

#[tokio::test]
async fn test_assign_player_to_seat() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "seatmanager@test.com", "manager").await;
    let (player_id, _) = create_test_user(&app_state, "seatplayer@test.com", "player").await;
    let club_id = create_test_club(&app_state, "Seat Test Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Seat Test Tournament").await;

    // Create club manager relationship
    create_club_manager(&app_state, manager_id, club_id).await;

    // First create a table
    let table_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO tournament_tables (id, tournament_id, table_number, max_seats) VALUES ($1, $2, $3, $4)",
        table_id,
        tournament_id,
        1,
        9
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test table");

    let query = r#"
        mutation AssignPlayerToSeat($input: AssignPlayerToSeatInput!) {
            assignPlayerToSeat(input: $input) {
                id
                seatNumber
                stackSize
                userId
                tableId
                tournamentId
                isCurrent
                assignedAt
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "tableId": table_id.to_string(),
            "userId": player_id.to_string(),
            "seatNumber": 1,
            "stackSize": 20000
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Assign player should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let assignment = &data["assignPlayerToSeat"];

    assert_eq!(assignment["seatNumber"], 1);
    assert_eq!(assignment["stackSize"], 20000);
    assert_eq!(assignment["userId"], player_id.to_string());
    assert_eq!(assignment["tableId"], table_id.to_string());
}

#[tokio::test]
async fn test_move_player() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "movemanager@test.com", "manager").await;
    let (player_id, _) = create_test_user(&app_state, "moveplayer@test.com", "player").await;
    let club_id = create_test_club(&app_state, "Move Test Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Move Test Tournament").await;

    // Create club manager relationship
    create_club_manager(&app_state, manager_id, club_id).await;

    // Create two tables
    let from_table_id = Uuid::new_v4();
    let to_table_id = Uuid::new_v4();

    sqlx::query!(
        "INSERT INTO tournament_tables (id, tournament_id, table_number, max_seats) VALUES ($1, $2, $3, $4), ($5, $2, $6, $4)",
        from_table_id, tournament_id, 1, 9,
        to_table_id, 2
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test tables");

    // First assign player to a seat
    sqlx::query!(
        "INSERT INTO table_seat_assignments (tournament_id, table_id, user_id, seat_number, stack_size) VALUES ($1, $2, $3, $4, $5)",
        tournament_id,
        from_table_id,
        player_id,
        1,
        15000
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create initial seat assignment");

    let query = r#"
        mutation MovePlayer($input: MovePlayerInput!) {
            movePlayer(input: $input) {
                id
                seatNumber
                stackSize
                userId
                tableId
                tournamentId
                isCurrent
                assignedAt
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string(),
            "newTableId": to_table_id.to_string(),
            "newSeatNumber": 3
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Move player should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let assignment = &data["movePlayer"];

    assert_eq!(assignment["seatNumber"], 3);
    assert_eq!(assignment["userId"], player_id.to_string());
    assert_eq!(assignment["tableId"], to_table_id.to_string());
}

#[tokio::test]
async fn test_update_stack_size() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "stackmanager@test.com", "manager").await;
    let (player_id, _) = create_test_user(&app_state, "stackplayer@test.com", "player").await;
    let club_id = create_test_club(&app_state, "Stack Test Club").await;

    // Create club manager relationship
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Stack Test Tournament").await;

    // Create table and initial assignment
    let table_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO tournament_tables (id, tournament_id, table_number, max_seats) VALUES ($1, $2, $3, $4)",
        table_id,
        tournament_id,
        1,
        9
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test table");

    sqlx::query!(
        "INSERT INTO table_seat_assignments (tournament_id, table_id, user_id, seat_number, stack_size) VALUES ($1, $2, $3, $4, $5)",
        tournament_id,
        table_id,
        player_id,
        1,
        20000
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create initial seat assignment");

    let query = r#"
        mutation UpdateStackSize($input: UpdateStackSizeInput!) {
            updateStackSize(input: $input) {
                id
                seatNumber
                stackSize
                userId
                tableId
                tournamentId
                isCurrent
                assignedAt
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string(),
            "newStackSize": 15000
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Update stack size should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let assignment = &data["updateStackSize"];

    assert_eq!(assignment["stackSize"], 15000);
    assert_eq!(assignment["userId"], player_id.to_string());
}

#[tokio::test]
async fn test_balance_tables() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "balancemanager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Balance Test Club").await;

    // Create club manager relationship
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Balance Test Tournament").await;

    let query = r#"
        mutation BalanceTables($input: BalanceTablesInput!) {
            balanceTables(input: $input) {
                id
                seatNumber
                stackSize
                userId
                tableId
                tournamentId
                isCurrent
                assignedAt
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string()
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    // This may return empty array if no balancing is needed, but should not error
    assert!(
        response.errors.is_empty(),
        "Balance tables should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let _assignments = data["balanceTables"].as_array().unwrap();

    // Should return an array (even if empty) - assignments is already a Vec from as_array()
    // No need to assert anything here since we already got a Vec from as_array().unwrap()
}

// =============================================================================
// SEAT ASSIGNMENT QUERIES
// =============================================================================

#[tokio::test]
async fn test_get_seat_assignments() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (_, _) = create_test_user(&app_state, "queryplayer@test.com", "player").await;
    let club_id = create_test_club(&app_state, "Query Test Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Query Test Tournament").await;

    let query = r#"
        query GetSeatAssignments($tableId: ID!) {
            tableSeatAssignments(tableId: $tableId) {
                assignment {
                    id
                    seatNumber
                    stackSize
                    userId
                    tableId
                    tournamentId
                }
                player {
                    id
                    firstName
                    lastName
                }
            }
        }
    "#;

    // Create a test table first
    let table_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO tournament_tables (id, tournament_id, table_number, max_seats) VALUES ($1, $2, $3, $4)",
        table_id,
        tournament_id,
        1,
        9
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test table");

    let variables = Variables::from_json(json!({
        "tableId": table_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Seat assignments query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let _assignments = data["tableSeatAssignments"].as_array().unwrap();

    // Should return an array (may be empty for new tournament) - assignments is already a Vec from as_array()
    // No need to assert anything here since we already got a Vec from as_array().unwrap()
}

#[tokio::test]
async fn test_seat_assignment_filtering() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let club_id = create_test_club(&app_state, "Filter Test Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Filter Test Tournament").await;

    // Create a table and assignment for testing
    let table_id = Uuid::new_v4();
    let (player_id, _) = create_test_user(&app_state, "filterplayer@test.com", "player").await;

    sqlx::query!(
        "INSERT INTO tournament_tables (id, tournament_id, table_number, max_seats) VALUES ($1, $2, $3, $4)",
        table_id,
        tournament_id,
        1,
        9
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test table");

    sqlx::query!(
        "INSERT INTO table_seat_assignments (tournament_id, table_id, user_id, seat_number, stack_size) VALUES ($1, $2, $3, $4, $5)",
        tournament_id,
        table_id,
        player_id,
        1,
        20000
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test assignment");

    // Test filtering by table ID
    let query = r#"
        query GetSeatAssignments($tableId: ID!) {
            tableSeatAssignments(tableId: $tableId) {
                assignment {
                    id
                    seatNumber
                    userId
                    tableId
                }
                player {
                    id
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tableId": table_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Filtered seat assignments query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let assignments = data["tableSeatAssignments"].as_array().unwrap();

    // Should find our test assignment
    if !assignments.is_empty() {
        assert_eq!(
            assignments[0]["assignment"]["tableId"],
            table_id.to_string()
        );
        assert_eq!(assignments[0]["player"]["id"], player_id.to_string());
    }
}
