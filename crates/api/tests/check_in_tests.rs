mod common;

use api::gql::build_schema;
use async_graphql::Variables;
use common::*;
use serde_json::json;

#[tokio::test]
async fn test_check_in_player_basic() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("checkin_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let (player_id, _) = create_test_user(
        &app_state,
        &format!("checkin_player_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Check-In Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Check-In Tournament").await;

    // Create a table and assign it to the tournament for auto-assignment
    let table_id = create_test_club_table(&app_state, club_id, 1, 9).await;
    assign_table_to_tournament(&app_state, tournament_id, table_id).await;

    // Register the player
    create_test_registration(&app_state, tournament_id, player_id, "registered").await;

    let query = r#"
        mutation CheckInPlayer($input: CheckInPlayerInput!) {
            checkInPlayer(input: $input) {
                registration {
                    id
                    status
                    userId
                }
                seatAssignment {
                    id
                    seatNumber
                    userId
                    clubTableId
                }
                message
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string()
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Check-in should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let result = &data["checkInPlayer"];

    assert_eq!(result["registration"]["status"], "CHECKED_IN");
    assert_eq!(result["registration"]["userId"], player_id.to_string());
    // With auto-assign (default), seat assignment should be populated
    assert!(
        !result["seatAssignment"].is_null(),
        "seatAssignment should be populated with auto-assign"
    );
}

#[tokio::test]
async fn test_check_in_player_no_auto_assign() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("noauto_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let (player_id, _) = create_test_user(
        &app_state,
        &format!("noauto_player_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "No Auto Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id = create_test_tournament(&app_state, club_id, "No Auto Tournament").await;

    // Register the player
    create_test_registration(&app_state, tournament_id, player_id, "registered").await;

    let query = r#"
        mutation CheckInPlayer($input: CheckInPlayerInput!) {
            checkInPlayer(input: $input) {
                registration {
                    id
                    status
                }
                seatAssignment {
                    id
                }
                message
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string(),
            "autoAssign": false
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Check-in with no auto-assign should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let result = &data["checkInPlayer"];

    assert_eq!(result["registration"]["status"], "CHECKED_IN");
    assert!(
        result["seatAssignment"].is_null(),
        "seatAssignment should be null when autoAssign is false"
    );
}

#[tokio::test]
async fn test_check_in_not_registered() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("noreg_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let (player_id, _) = create_test_user(
        &app_state,
        &format!("noreg_player_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "No Reg Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id = create_test_tournament(&app_state, club_id, "No Reg Tournament").await;

    // Do NOT register the player

    let query = r#"
        mutation CheckInPlayer($input: CheckInPlayerInput!) {
            checkInPlayer(input: $input) {
                registration { id }
                message
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string()
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Check-in of unregistered player should fail"
    );
    let error_msg = &response.errors[0].message.to_lowercase();
    assert!(
        error_msg.contains("not registered")
            || error_msg.contains("no registration")
            || error_msg.contains("not found"),
        "Expected 'not registered' error, got: '{}'",
        response.errors[0].message
    );
}

#[tokio::test]
async fn test_check_in_already_checked_in() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("dblcheckin_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let (player_id, _) = create_test_user(
        &app_state,
        &format!("dblcheckin_player_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Double CheckIn Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Double CheckIn Tournament").await;

    // Register as already checked_in
    create_test_registration(&app_state, tournament_id, player_id, "checked_in").await;

    let query = r#"
        mutation CheckInPlayer($input: CheckInPlayerInput!) {
            checkInPlayer(input: $input) {
                registration { id }
                message
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string()
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Check-in of already checked-in player should fail"
    );
    let error_msg = &response.errors[0].message.to_lowercase();
    assert!(
        error_msg.contains("cannot be checked in from status")
            || error_msg.contains("already checked in")
            || error_msg.contains("checked_in"),
        "Expected status error, got: '{}'",
        response.errors[0].message
    );
}

#[tokio::test]
async fn test_check_in_unauthorized() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (player_id, player_claims) = create_test_user(
        &app_state,
        &format!("unauth_checkin_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Unauth CheckIn Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Unauth CheckIn Tournament").await;

    create_test_registration(&app_state, tournament_id, player_id, "registered").await;

    let query = r#"
        mutation CheckInPlayer($input: CheckInPlayerInput!) {
            checkInPlayer(input: $input) {
                registration { id }
                message
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string()
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(player_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Player should not be able to check in others"
    );
    assert!(
        response.errors[0].message.contains("Access denied"),
        "Expected access denied error, got: '{}'",
        response.errors[0].message
    );
}
