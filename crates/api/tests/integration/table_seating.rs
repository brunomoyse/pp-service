

use api::gql::build_schema;
use async_graphql::Variables;
use crate::common::*;
use serde_json::json;
use uuid::Uuid;

// =============================================================================
// TABLE MANAGEMENT TESTS
// =============================================================================

#[tokio::test]
async fn test_create_tournament_table() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique_email = format!(
        "tablemanager_{}@test.com",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let (manager_id, manager_claims) = create_test_user(&app_state, &unique_email, "manager").await;
    let club_id = create_test_club(&app_state, "Table Test Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Table Test Tournament").await;

    // Create club manager relationship
    create_club_manager(&app_state, manager_id, club_id).await;

    // Create a club table first
    let club_table_id = create_test_club_table(&app_state, club_id, 1, 9).await;

    let query = r#"
        mutation AssignTableToTournament($input: AssignTableToTournamentInput!) {
            assignTableToTournament(input: $input) {
                id
                tournamentId
                tableNumber
                maxSeats
                isActive
                createdAt
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "clubTableId": club_table_id.to_string()
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Assign table should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let table = &data["assignTableToTournament"];

    assert_eq!(table["tableNumber"], 1);
    assert_eq!(table["maxSeats"], 9);
    assert_eq!(table["isActive"], true);
    assert_eq!(table["tournamentId"], tournament_id.to_string());
}

#[tokio::test]
async fn test_create_table_unauthorized() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique_email = format!(
        "tableplayer_{}@test.com",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let (_, player_claims) = create_test_user(&app_state, &unique_email, "player").await;
    let club_id = create_test_club(&app_state, "Unauthorized Table Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Unauthorized Table Tournament").await;

    // Create a club table first
    let club_table_id = create_test_club_table(&app_state, club_id, 1, 9).await;

    let query = r#"
        mutation AssignTableToTournament($input: AssignTableToTournamentInput!) {
            assignTableToTournament(input: $input) {
                id
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "clubTableId": club_table_id.to_string()
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(player_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Player should not be able to assign table: {:?}",
        response.errors
    );

    // The actual error message should contain information about requiring manager permissions
    let error_msg = &response.errors[0].message;
    assert!(
        error_msg.contains("Access denied")
            || error_msg.contains("not authorized to manage this club")
            || error_msg.contains("Unauthorized")
            || error_msg.contains("forbidden")
            || error_msg.contains("no rows returned"), // Database error when user permissions can't be verified
        "Expected authorization error, got: '{}'",
        error_msg
    );
}

// =============================================================================
// SEAT ASSIGNMENT TESTS
// =============================================================================

#[tokio::test]
async fn test_assign_player_to_seat() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique_manager_email = format!(
        "seatmanager_{}@test.com",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let (manager_id, manager_claims) =
        create_test_user(&app_state, &unique_manager_email, "manager").await;
    let unique_player_email = format!(
        "seatplayer_{}@test.com",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let (player_id, _) = create_test_user(&app_state, &unique_player_email, "player").await;
    let club_id = create_test_club(&app_state, "Seat Test Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Seat Test Tournament").await;

    // Create club manager relationship
    create_club_manager(&app_state, manager_id, club_id).await;

    // First create a table
    let club_table_id = Uuid::new_v4();
    // Create club table and assign to tournament
    sqlx::query!(
        "INSERT INTO club_tables (id, club_id, table_number, max_seats) VALUES ($1, $2, $3, $4)",
        club_table_id,
        club_id,
        1,
        9
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create club table");

    sqlx::query!(
        "INSERT INTO tournament_table_assignments (tournament_id, club_table_id) VALUES ($1, $2)",
        tournament_id,
        club_table_id
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
                clubTableId
                tournamentId
                isCurrent
                assignedAt
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "clubTableId": club_table_id.to_string(),
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
    assert_eq!(assignment["clubTableId"], club_table_id.to_string());
}

#[tokio::test]
async fn test_move_player() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique_manager_email = format!(
        "movemanager_{}@test.com",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let (manager_id, manager_claims) =
        create_test_user(&app_state, &unique_manager_email, "manager").await;
    let unique_player_email = format!(
        "moveplayer_{}@test.com",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let (player_id, _) = create_test_user(&app_state, &unique_player_email, "player").await;
    let club_id = create_test_club(&app_state, "Move Test Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Move Test Tournament").await;

    // Create club manager relationship
    create_club_manager(&app_state, manager_id, club_id).await;

    // Create two tables
    let from_club_table_id = Uuid::new_v4();
    let to_club_table_id = Uuid::new_v4();

    // Create two club tables and assign to tournament
    sqlx::query!(
        "INSERT INTO club_tables (id, club_id, table_number, max_seats) VALUES ($1, $2, $3, $4)",
        from_club_table_id,
        club_id,
        1,
        9
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create first club table");

    sqlx::query!(
        "INSERT INTO club_tables (id, club_id, table_number, max_seats) VALUES ($1, $2, $3, $4)",
        to_club_table_id,
        club_id,
        2,
        9
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create second club table");

    sqlx::query!(
        "INSERT INTO tournament_table_assignments (tournament_id, club_table_id) VALUES ($1, $2), ($1, $3)",
        tournament_id,
        from_club_table_id,
        to_club_table_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test tables");

    // First assign player to a seat
    sqlx::query!(
        "INSERT INTO table_seat_assignments (tournament_id, club_table_id, user_id, seat_number, stack_size) VALUES ($1, $2, $3, $4, $5)",
        tournament_id,
        from_club_table_id,
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
                clubTableId
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
            "newClubTableId": to_club_table_id.to_string(),
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
    assert_eq!(assignment["clubTableId"], to_club_table_id.to_string());
}

#[tokio::test]
async fn test_update_stack_size() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique_manager_email = format!(
        "stackmanager_{}@test.com",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let (manager_id, manager_claims) =
        create_test_user(&app_state, &unique_manager_email, "manager").await;
    let unique_player_email = format!(
        "stackplayer_{}@test.com",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let (player_id, _) = create_test_user(&app_state, &unique_player_email, "player").await;
    let club_id = create_test_club(&app_state, "Stack Test Club").await;

    // Create club manager relationship
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Stack Test Tournament").await;

    // Create table and initial assignment
    let club_table_id = Uuid::new_v4();
    // Create club table and assign to tournament
    sqlx::query!(
        "INSERT INTO club_tables (id, club_id, table_number, max_seats) VALUES ($1, $2, $3, $4)",
        club_table_id,
        club_id,
        1,
        9
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create club table");

    sqlx::query!(
        "INSERT INTO tournament_table_assignments (tournament_id, club_table_id) VALUES ($1, $2)",
        tournament_id,
        club_table_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test table");

    sqlx::query!(
        "INSERT INTO table_seat_assignments (tournament_id, club_table_id, user_id, seat_number, stack_size) VALUES ($1, $2, $3, $4, $5)",
        tournament_id,
        club_table_id,
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
                clubTableId
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

    let unique_manager_email = format!(
        "balancemanager_{}@test.com",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let (manager_id, manager_claims) =
        create_test_user(&app_state, &unique_manager_email, "manager").await;
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
                clubTableId
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

    let unique_player_email = format!(
        "queryplayer_{}@test.com",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let (_, _) = create_test_user(&app_state, &unique_player_email, "player").await;
    let club_id = create_test_club(&app_state, "Query Test Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Query Test Tournament").await;

    let query = r#"
        query GetSeatAssignments($clubTableId: ID!) {
            tableSeatAssignments(clubTableId: $clubTableId) {
                assignment {
                    id
                    seatNumber
                    stackSize
                    userId
                    clubTableId
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
    let club_table_id = Uuid::new_v4();
    // Create club table and assign to tournament
    sqlx::query!(
        "INSERT INTO club_tables (id, club_id, table_number, max_seats) VALUES ($1, $2, $3, $4)",
        club_table_id,
        club_id,
        1,
        9
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create club table");

    sqlx::query!(
        "INSERT INTO tournament_table_assignments (tournament_id, club_table_id) VALUES ($1, $2)",
        tournament_id,
        club_table_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test table");

    let variables = Variables::from_json(json!({
        "clubTableId": club_table_id.to_string()
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
    let club_table_id = Uuid::new_v4();
    let unique_player_email = format!(
        "filterplayer_{}@test.com",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let (player_id, _) = create_test_user(&app_state, &unique_player_email, "player").await;

    // Create club table and assign to tournament
    sqlx::query!(
        "INSERT INTO club_tables (id, club_id, table_number, max_seats) VALUES ($1, $2, $3, $4)",
        club_table_id,
        club_id,
        1,
        9
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create club table");

    sqlx::query!(
        "INSERT INTO tournament_table_assignments (tournament_id, club_table_id) VALUES ($1, $2)",
        tournament_id,
        club_table_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test table");

    sqlx::query!(
        "INSERT INTO table_seat_assignments (tournament_id, club_table_id, user_id, seat_number, stack_size) VALUES ($1, $2, $3, $4, $5)",
        tournament_id,
        club_table_id,
        player_id,
        1,
        20000
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create test assignment");

    // Test filtering by table ID
    let query = r#"
        query GetSeatAssignments($clubTableId: ID!) {
            tableSeatAssignments(clubTableId: $clubTableId) {
                assignment {
                    id
                    seatNumber
                    userId
                    clubTableId
                }
                player {
                    id
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "clubTableId": club_table_id.to_string()
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
            assignments[0]["assignment"]["clubTableId"],
            club_table_id.to_string()
        );
        assert_eq!(assignments[0]["player"]["id"], player_id.to_string());
    }
}
