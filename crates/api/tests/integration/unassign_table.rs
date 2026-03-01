

use api::gql::build_schema;
use async_graphql::Variables;
use crate::common::*;
use serde_json::json;

#[tokio::test]
async fn test_unassign_table_success() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("unassign_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Unassign Table Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Unassign Table Tournament").await;

    // Create and assign a table
    let table_id = create_test_club_table(&app_state, club_id, 1, 9).await;
    assign_table_to_tournament(&app_state, tournament_id, table_id).await;

    let query = r#"
        mutation UnassignTableFromTournament($input: UnassignTableFromTournamentInput!) {
            unassignTableFromTournament(input: $input)
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
        query,
        Some(variables),
        Some(manager_claims.clone()),
    )
    .await;

    assert!(
        response.errors.is_empty(),
        "Unassign table should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["unassignTableFromTournament"], true);

    // Verify the table is gone from tournamentTables
    let tables_query = r#"
        query TournamentTables($tournamentId: ID!) {
            tournamentTables(tournamentId: $tournamentId) {
                id
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    let response =
        execute_graphql(&schema, tables_query, Some(variables), Some(manager_claims)).await;
    assert!(
        response.errors.is_empty(),
        "Tables query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let tables = data["tournamentTables"].as_array().unwrap();
    assert!(
        tables.is_empty(),
        "Tournament should have no tables after unassign"
    );
}

#[tokio::test]
async fn test_unassign_table_with_seated_players() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("unassignseated_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let (player_id, _) = create_test_user(
        &app_state,
        &format!("unassignseated_player_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Seated Unassign Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Seated Unassign Tournament").await;

    // Create table, assign it, and seat a player
    let table_id = create_test_club_table(&app_state, club_id, 1, 9).await;
    assign_table_to_tournament(&app_state, tournament_id, table_id).await;

    sqlx::query!(
        "INSERT INTO table_seat_assignments (tournament_id, club_table_id, user_id, seat_number, stack_size) VALUES ($1, $2, $3, $4, $5)",
        tournament_id,
        table_id,
        player_id,
        1,
        20000
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to seat player");

    let query = r#"
        mutation UnassignTableFromTournament($input: UnassignTableFromTournamentInput!) {
            unassignTableFromTournament(input: $input)
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "clubTableId": table_id.to_string()
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Unassigning table with seated players should fail"
    );
    let error_msg = &response.errors[0].message.to_lowercase();
    assert!(
        error_msg.contains("still players seated")
            || error_msg.contains("players are still")
            || error_msg.contains("seated players")
            || error_msg.contains("has active seat"),
        "Expected seated players error, got: '{}'",
        response.errors[0].message
    );
}

#[tokio::test]
async fn test_unassign_table_unauthorized() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (_, player_claims) = create_test_user(
        &app_state,
        &format!("unassignunauth_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Unauth Unassign Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Unauth Unassign Tournament").await;

    let table_id = create_test_club_table(&app_state, club_id, 1, 9).await;
    assign_table_to_tournament(&app_state, tournament_id, table_id).await;

    let query = r#"
        mutation UnassignTableFromTournament($input: UnassignTableFromTournamentInput!) {
            unassignTableFromTournament(input: $input)
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "clubTableId": table_id.to_string()
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(player_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Player should not be able to unassign tables"
    );
    let error_msg = &response.errors[0].message;
    assert!(
        error_msg.contains("Access denied")
            || error_msg.contains("not authorized to manage this club"),
        "Expected authorization error, got: '{}'",
        error_msg
    );
}
