mod common;

use api::gql::build_schema;
use async_graphql::Variables;
use common::*;
use serde_json::json;

#[tokio::test]
async fn test_create_tournament_clock() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "clockmanager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Clock Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Clock Tournament").await;

    // Create club manager relationship
    create_club_manager(&app_state, manager_id, club_id).await;

    // Create tournament structures first
    sqlx::query!(
        r#"INSERT INTO tournament_structures (tournament_id, level_number, small_blind, big_blind, ante, duration_minutes) 
           VALUES 
           ($1, 1, 25, 50, 0, 20),
           ($1, 2, 50, 100, 0, 20),
           ($1, 3, 75, 150, 25, 20)
           ON CONFLICT DO NOTHING"#,
        tournament_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create tournament structures");

    let query = r#"
        mutation CreateTournamentClock($tournamentId: ID!) {
            createTournamentClock(tournamentId: $tournamentId) {
                id
                tournamentId
                status
                currentLevel
                timeRemainingSeconds
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    let response = execute_graphql(
        &schema,
        query,
        Some(variables.clone()),
        Some(manager_claims.clone()),
    )
    .await;

    // Clock creation might fail if it already exists, which is OK
    if !response.errors.is_empty() && !response.errors[0].message.contains("duplicate key") {
        panic!(
            "Create clock should succeed or already exist: {:?}",
            response.errors
        );
    }

    // If there were errors, it means the clock already existed, so we need to query it
    let clock_data = if response.errors.is_empty() {
        response.data.into_json().unwrap()
    } else {
        // Query the existing clock
        let query_clock = r#"
            query GetTournamentClock($tournamentId: ID!) {
                tournamentClock(tournamentId: $tournamentId) {
                    id
                    tournamentId
                    status
                    currentLevel
                    timeRemainingSeconds
                }
            }
        "#;

        let query_response = execute_graphql(
            &schema,
            query_clock,
            Some(variables.clone()),
            Some(manager_claims.clone()),
        )
        .await;
        assert!(
            query_response.errors.is_empty(),
            "Query clock should succeed: {:?}",
            query_response.errors
        );
        query_response.data.into_json().unwrap()
    };

    let clock = if response.errors.is_empty() {
        &clock_data["createTournamentClock"]
    } else {
        &clock_data["tournamentClock"]
    };

    assert_eq!(clock["tournamentId"], tournament_id.to_string());
    assert_eq!(clock["status"], "STOPPED");
    assert_eq!(clock["currentLevel"], 1);
}

#[tokio::test]
async fn test_advance_tournament_level() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "advancemanager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Advance Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Advance Tournament").await;

    // Create club manager relationship
    create_club_manager(&app_state, manager_id, club_id).await;

    // Create tournament structures and clock
    sqlx::query!(
        r#"INSERT INTO tournament_structures (tournament_id, level_number, small_blind, big_blind, ante, duration_minutes) 
           VALUES 
           ($1, 1, 25, 50, 0, 20),
           ($1, 2, 50, 100, 0, 20)
           ON CONFLICT DO NOTHING"#,
        tournament_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create tournament structures");

    // Create clock first
    let create_query = r#"
        mutation CreateTournamentClock($tournamentId: ID!) {
            createTournamentClock(tournamentId: $tournamentId) {
                id
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    let response = execute_graphql(
        &schema,
        create_query,
        Some(variables.clone()),
        Some(manager_claims.clone()),
    )
    .await;
    assert!(
        response.errors.is_empty() || response.errors[0].message.contains("duplicate key"),
        "Create clock should succeed or already exist: {:?}",
        response.errors
    );

    // Now advance level
    let advance_query = r#"
        mutation AdvanceTournamentLevel($tournamentId: ID!) {
            advanceTournamentLevel(tournamentId: $tournamentId) {
                id
                tournamentId
                currentLevel
                status
            }
        }
    "#;

    let response = execute_graphql(
        &schema,
        advance_query,
        Some(variables),
        Some(manager_claims),
    )
    .await;

    assert!(
        response.errors.is_empty(),
        "Advance level should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let clock = &data["advanceTournamentLevel"];

    assert_eq!(clock["currentLevel"], 2);
}

#[tokio::test]
async fn test_revert_tournament_level() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "revertmanager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Revert Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Revert Tournament").await;

    // Create club manager relationship
    create_club_manager(&app_state, manager_id, club_id).await;

    // Create tournament structures and clock
    sqlx::query!(
        r#"INSERT INTO tournament_structures (tournament_id, level_number, small_blind, big_blind, ante, duration_minutes) 
           VALUES 
           ($1, 1, 25, 50, 0, 20),
           ($1, 2, 50, 100, 0, 20)
           ON CONFLICT DO NOTHING"#,
        tournament_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create tournament structures");

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    // Create clock and advance to level 2
    let create_query = r#"
        mutation CreateTournamentClock($tournamentId: ID!) {
            createTournamentClock(tournamentId: $tournamentId) {
                id
            }
        }
    "#;

    execute_graphql(
        &schema,
        create_query,
        Some(variables.clone()),
        Some(manager_claims.clone()),
    )
    .await;

    let advance_query = r#"
        mutation AdvanceTournamentLevel($tournamentId: ID!) {
            advanceTournamentLevel(tournamentId: $tournamentId) {
                id
            }
        }
    "#;

    execute_graphql(
        &schema,
        advance_query,
        Some(variables.clone()),
        Some(manager_claims.clone()),
    )
    .await;

    // Now revert
    let revert_query = r#"
        mutation RevertTournamentLevel($tournamentId: ID!) {
            revertTournamentLevel(tournamentId: $tournamentId) {
                id
                tournamentId
                currentLevel
                status
            }
        }
    "#;

    let response = execute_graphql(
        &schema,
        revert_query,
        Some(variables.clone()),
        Some(manager_claims.clone()),
    )
    .await;

    assert!(
        response.errors.is_empty(),
        "Revert level should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let clock = &data["revertTournamentLevel"];

    assert_eq!(clock["currentLevel"], 1);

    // Try to revert below level 1 (should fail)
    let response =
        execute_graphql(&schema, revert_query, Some(variables), Some(manager_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Revert below level 1 should fail"
    );
    assert!(response.errors[0]
        .message
        .contains("Cannot revert: Tournament is already at level 1"));
}

#[tokio::test]
async fn test_tournament_clock_unauthorized() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (_, player_claims) = create_test_user(&app_state, "player@test.com", "player").await;
    let club_id = create_test_club(&app_state, "Unauthorized Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Unauthorized Tournament").await;

    let query = r#"
        mutation CreateTournamentClock($tournamentId: ID!) {
            createTournamentClock(tournamentId: $tournamentId) {
                id
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(player_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Player should not be able to create tournament clock"
    );
    assert!(response.errors[0]
        .message
        .contains("Manager privileges required"));
}
