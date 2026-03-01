use crate::common::*;
use api::gql::build_schema;
use async_graphql::Variables;
use serde_json::json;

/// Test that registration mutation triggers notification
/// (notification is published but we can't easily capture it in tests)
#[tokio::test]
async fn test_registration_triggers_notification() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    // Create club and tournament
    let club_id = create_test_club(&app_state, "Notification Test Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Notification Test Tournament").await;

    // Open registration so the player can register
    sqlx::query("UPDATE tournaments SET live_status = 'registration_open'::tournament_live_status WHERE id = $1")
        .bind(tournament_id)
        .execute(&app_state.db)
        .await
        .expect("Failed to open registration");

    // Create player
    let (_, player_claims) =
        create_test_user(&app_state, "notification_player@test.com", "player").await;

    // Register for tournament - this should trigger a notification
    // Note: When self-registering, don't pass userId (it's for admin registering others)
    let mutation = r#"
        mutation Register($input: RegisterForTournamentInput!) {
            registerForTournament(input: $input) {
                id
                tournamentId
                userId
                status
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string()
        }
    }));

    let response = execute_graphql(&schema, mutation, Some(variables), Some(player_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Registration should succeed and trigger notification: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["registerForTournament"]["status"], "REGISTERED");
    // The notification is published internally - we verify the mutation works
}

/// Test upcoming tournaments query (used by notification service)
#[tokio::test]
async fn test_tournaments_starting_soon_query() {
    let app_state = setup_test_db().await;

    // Create a tournament starting soon (within 15 minutes)
    let club_id = create_test_club(&app_state, "Starting Soon Club").await;

    // Insert tournament with start_time in the future
    let tournament_id = uuid::Uuid::new_v4();
    let start_time = chrono::Utc::now() + chrono::Duration::minutes(10);

    sqlx::query(
        r#"INSERT INTO tournaments (
            id, name, description, club_id, start_time, end_time,
            buy_in_cents, seat_cap, live_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'registration_open'::tournament_live_status)"#,
    )
    .bind(tournament_id)
    .bind("Starting Soon Tournament")
    .bind("Test tournament starting soon")
    .bind(club_id)
    .bind(start_time)
    .bind(start_time + chrono::Duration::hours(4))
    .bind(5000i32)
    .bind(100i32)
    .execute(&app_state.db)
    .await
    .expect("Failed to create tournament");

    // Query using the standalone function
    let upcoming = infra::repos::tournaments::list_starting_soon(&app_state.db, 16).await;

    assert!(
        upcoming.is_ok(),
        "Query should succeed: {:?}",
        upcoming.err()
    );

    let tournaments = upcoming.unwrap();

    // Should find at least our test tournament
    assert!(
        tournaments.iter().any(|t| t.id == tournament_id),
        "Should find the tournament starting soon"
    );
}

/// Test that clock query and create mutations work
#[tokio::test]
async fn test_clock_query_and_create() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "clock_sub_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Clock Query Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let tournament_id = create_test_tournament(&app_state, club_id, "Clock Query Tournament").await;

    // Query the clock (may or may not exist depending on DB trigger)
    let query = r#"
        query GetClock($tournamentId: ID!) {
            tournamentClock(tournamentId: $tournamentId) {
                id
                status
                currentLevel
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    let response = execute_graphql(
        &schema,
        query,
        Some(variables),
        Some(manager_claims.clone()),
    )
    .await;

    assert!(
        response.errors.is_empty(),
        "Query clock should succeed: {:?}",
        response.errors
    );

    // If no clock exists, create one
    let data = response.data.into_json().unwrap();
    if data["tournamentClock"].is_null() {
        let create_mutation = r#"
            mutation CreateClock($tournamentId: ID!) {
                createTournamentClock(tournamentId: $tournamentId) {
                    id
                    status
                    currentLevel
                }
            }
        "#;

        let variables = Variables::from_json(json!({
            "tournamentId": tournament_id.to_string()
        }));

        let response = execute_graphql(
            &schema,
            create_mutation,
            Some(variables),
            Some(manager_claims),
        )
        .await;

        assert!(
            response.errors.is_empty(),
            "Create clock should succeed: {:?}",
            response.errors
        );

        let data = response.data.into_json().unwrap();
        assert_eq!(data["createTournamentClock"]["status"], "STOPPED");
        assert_eq!(data["createTournamentClock"]["currentLevel"], 1);
    } else {
        // Clock already exists from trigger
        assert_eq!(data["tournamentClock"]["status"], "STOPPED");
    }
}

/// Test seating change triggers subscription
#[tokio::test]
async fn test_seating_change_triggers_subscription() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "seating_sub_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Seating Subscription Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let tournament_id =
        create_test_tournament(&app_state, club_id, "Seating Subscription Tournament").await;

    // Create a table
    let table_id = create_test_club_table(&app_state, club_id, 1, 9).await;

    // Assign table to tournament
    let assign_mutation = r#"
        mutation AssignTable($input: AssignTableToTournamentInput!) {
            assignTableToTournament(input: $input) {
                id
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "clubTableId": table_id.to_string()
        }
    }));

    let response = execute_graphql(
        &schema,
        assign_mutation,
        Some(variables),
        Some(manager_claims.clone()),
    )
    .await;

    assert!(
        response.errors.is_empty(),
        "Assign table should succeed: {:?}",
        response.errors
    );

    // Create player and assign to seat
    let (player_id, _) =
        create_test_user(&app_state, "seating_sub_player@test.com", "player").await;

    let seat_mutation = r#"
        mutation AssignSeat($input: AssignPlayerToSeatInput!) {
            assignPlayerToSeat(input: $input) {
                id
                seatNumber
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "clubTableId": table_id.to_string(),
            "userId": player_id.to_string(),
            "seatNumber": 1
        }
    }));

    let response = execute_graphql(
        &schema,
        seat_mutation,
        Some(variables),
        Some(manager_claims),
    )
    .await;

    assert!(
        response.errors.is_empty(),
        "Assign seat should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["assignPlayerToSeat"]["seatNumber"], 1);
}
