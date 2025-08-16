mod common;

use api::gql::build_schema;
use async_graphql::Variables;
use common::*;
use serde_json::json;

#[tokio::test]
async fn test_role_based_permissions() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    // Test player role limitations
    let (_, player_claims) = create_test_user(&app_state, "testplayer@test.com", "player").await;
    let club_id = create_test_club(&app_state, "Permission Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Permission Tournament").await;

    let manager_query = r#"
        mutation AdvanceTournamentLevel($tournamentId: ID!) {
            advanceTournamentLevel(tournamentId: $tournamentId) {
                id
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    // Player should not be able to advance tournament level
    let response = execute_graphql(
        &schema,
        manager_query,
        Some(variables.clone()),
        Some(player_claims),
    )
    .await;

    assert!(
        !response.errors.is_empty(),
        "Player should not have manager permissions"
    );
    assert!(response.errors[0]
        .message
        .contains("Manager privileges required"));
    assert!(response.errors[0]
        .message
        .contains("Your current role is Player"));

    // Manager should be able to advance tournament level (after creating clock and structures)
    let (manager_id, manager_claims) =
        create_test_user(&app_state, "testmanager@test.com", "manager").await;

    // Create club manager relationship
    create_club_manager(&app_state, manager_id, club_id).await;

    // First create structures and clock
    sqlx::query!(
        r#"INSERT INTO tournament_structures (tournament_id, level_number, small_blind, big_blind, ante, duration_minutes) 
           VALUES ($1, 1, 25, 50, 0, 20) ON CONFLICT DO NOTHING"#,
        tournament_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create tournament structure");

    // Create clock first
    let create_clock_query = r#"
        mutation CreateTournamentClock($tournamentId: ID!) {
            createTournamentClock(tournamentId: $tournamentId) {
                id
            }
        }
    "#;

    let response = execute_graphql(
        &schema,
        create_clock_query,
        Some(variables.clone()),
        Some(manager_claims.clone()),
    )
    .await;
    assert!(
        response.errors.is_empty() || response.errors[0].message.contains("duplicate key"),
        "Create clock should succeed or already exist: {:?}",
        response.errors
    );

    // Now advance should work
    let response = execute_graphql(
        &schema,
        manager_query,
        Some(variables),
        Some(manager_claims),
    )
    .await;

    // This might fail due to missing tournament structures, but the authorization should pass
    if !response.errors.is_empty() {
        // Should not be an authorization error
        assert!(!response.errors[0]
            .message
            .contains("Manager privileges required"));
    }
}

#[tokio::test]
async fn test_admin_vs_manager_permissions() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (_, admin_claims) = create_test_user(&app_state, "testadmin@test.com", "admin").await;
    let (_, manager_claims) =
        create_test_user(&app_state, "testmanager2@test.com", "manager").await;

    // Test users query - admin should have access
    let users_query = r#"
        query GetUsers($limit: Int) {
            users(limit: $limit) {
                id
                email
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "limit": 5
    }));

    // Admin should be able to access users
    let response = execute_graphql(
        &schema,
        users_query,
        Some(variables.clone()),
        Some(admin_claims),
    )
    .await;
    assert!(
        response.errors.is_empty(),
        "Admin should be able to access users: {:?}",
        response.errors
    );

    // Manager might have limited access (depending on implementation)
    let _response =
        execute_graphql(&schema, users_query, Some(variables), Some(manager_claims)).await;
    // This test depends on your specific permission implementation
    // Either it should succeed (if managers can view users) or fail with proper error message
}

#[tokio::test]
async fn test_club_manager_restrictions() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "clubmanager_restrictions@test.com", "manager").await;
    let club_a_id = create_test_club(&app_state, "Club A").await;
    let club_b_id = create_test_club(&app_state, "Club B").await;

    // Manager is only associated with Club A
    create_club_manager(&app_state, manager_id, club_a_id).await;

    let tournament_a_id = create_test_tournament(&app_state, club_a_id, "Tournament A").await;
    let tournament_b_id = create_test_tournament(&app_state, club_b_id, "Tournament B").await;

    let advance_query = r#"
        mutation AdvanceTournamentLevel($tournamentId: ID!) {
            advanceTournamentLevel(tournamentId: $tournamentId) {
                id
            }
        }
    "#;

    // Should be able to manage tournaments in Club A
    let _variables_a = Variables::from_json(json!({
        "tournamentId": tournament_a_id.to_string()
    }));

    // Should NOT be able to manage tournaments in Club B
    let variables_b = Variables::from_json(json!({
        "tournamentId": tournament_b_id.to_string()
    }));

    let response_b = execute_graphql(
        &schema,
        advance_query,
        Some(variables_b),
        Some(manager_claims),
    )
    .await;

    assert!(
        !response_b.errors.is_empty(),
        "Manager should not be able to manage tournaments in other clubs"
    );
    assert!(
        response_b.errors[0]
            .message
            .contains("not authorized to manage this club")
            || response_b.errors[0]
                .message
                .contains("Manager privileges required")
    );
}
