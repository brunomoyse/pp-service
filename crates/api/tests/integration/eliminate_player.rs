use crate::common::*;
use api::gql::build_schema;
use async_graphql::Variables;
use serde_json::json;

#[tokio::test]
async fn test_eliminate_player_success() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("elim_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let (player_id, _) = create_test_user(
        &app_state,
        &format!("elim_player_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Eliminate Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Eliminate Tournament").await;

    // Create table, assign to tournament, seat the player
    let table_id = create_test_club_table(&app_state, club_id, 1, 9).await;
    assign_table_to_tournament(&app_state, tournament_id, table_id).await;

    sqlx::query!(
        "INSERT INTO table_seat_assignments (tournament_id, club_table_id, user_id, seat_number, stack_size) VALUES ($1, $2, $3, $4, $5)",
        tournament_id,
        table_id,
        player_id,
        1,
        15000
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create seat assignment");

    let query = r#"
        mutation EliminatePlayer($tournamentId: ID!, $userId: ID!) {
            eliminatePlayer(tournamentId: $tournamentId, userId: $userId)
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string(),
        "userId": player_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Eliminate player should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["eliminatePlayer"], true);

    // Verify player is no longer seated (is_current = false)
    let row = sqlx::query!(
        "SELECT is_current FROM table_seat_assignments WHERE tournament_id = $1 AND user_id = $2 ORDER BY assigned_at DESC LIMIT 1",
        tournament_id,
        player_id
    )
    .fetch_one(&app_state.db)
    .await
    .expect("Should find seat assignment record");

    assert!(
        !row.is_current,
        "Player should no longer be currently seated"
    );
}

#[tokio::test]
async fn test_eliminate_player_not_seated() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("elimnoseat_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let (player_id, _) = create_test_user(
        &app_state,
        &format!("elimnoseat_player_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Elim No Seat Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Elim No Seat Tournament").await;

    // Player is NOT seated

    let query = r#"
        mutation EliminatePlayer($tournamentId: ID!, $userId: ID!) {
            eliminatePlayer(tournamentId: $tournamentId, userId: $userId)
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string(),
        "userId": player_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Eliminating unseated player should fail"
    );
    let error_msg = &response.errors[0].message.to_lowercase();
    assert!(
        error_msg.contains("not currently assigned to a seat")
            || error_msg.contains("no current seat")
            || error_msg.contains("not seated")
            || error_msg.contains("not found"),
        "Expected seat-related error, got: '{}'",
        response.errors[0].message
    );
}

#[tokio::test]
async fn test_eliminate_player_unauthorized() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (player_id, player_claims) = create_test_user(
        &app_state,
        &format!("elimunauth_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Elim Unauth Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Elim Unauth Tournament").await;

    let query = r#"
        mutation EliminatePlayer($tournamentId: ID!, $userId: ID!) {
            eliminatePlayer(tournamentId: $tournamentId, userId: $userId)
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string(),
        "userId": player_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(player_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Player should not be able to eliminate"
    );
    assert!(
        response.errors[0].message.contains("Access denied"),
        "Expected access denied error, got: '{}'",
        response.errors[0].message
    );
}
