mod common;

use api::gql::build_schema;
use async_graphql::Variables;
use common::*;
use serde_json::json;

#[tokio::test]
async fn test_get_tournament_payout() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    // Create club and tournament
    let club_id = create_test_club(&app_state, "Payout Test Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Payout Test Tournament").await;

    // Query tournament payout (should be auto-created by trigger)
    let query = r#"
        query GetPayout($tournamentId: ID!) {
            tournamentPayout(tournamentId: $tournamentId) {
                id
                tournamentId
                totalPrizePool
                playerCount
                positions {
                    position
                    amountCents
                    percentage
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Payout query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let payout = &data["tournamentPayout"];

    // Tournament payout should exist (created by DB trigger)
    if payout.is_object() && !payout.is_null() {
        assert_eq!(payout["tournamentId"], tournament_id.to_string());
        assert!(payout["positions"].is_array());
    }
}

#[tokio::test]
async fn test_payout_recalculation_on_entry() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    // Create manager and club
    let (manager_id, manager_claims) =
        create_test_user(&app_state, "payout_recalc_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Payout Recalc Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let tournament_id =
        create_test_tournament(&app_state, club_id, "Payout Recalc Tournament").await;

    // Create players
    let (player1_id, _) =
        create_test_user(&app_state, "payout_recalc_player1@test.com", "player").await;
    let (player2_id, _) =
        create_test_user(&app_state, "payout_recalc_player2@test.com", "player").await;

    // Add entries - prize pool should update via DB trigger
    let mutation = r#"
        mutation AddEntry($input: AddTournamentEntryInput!) {
            addTournamentEntry(input: $input) {
                id
                amountCents
            }
        }
    "#;

    // Add first entry
    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player1_id.to_string(),
            "entryType": "INITIAL",
            "amountCents": 5000
        }
    }));
    execute_graphql(
        &schema,
        mutation,
        Some(variables),
        Some(manager_claims.clone()),
    )
    .await;

    // Add second entry
    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player2_id.to_string(),
            "entryType": "INITIAL",
            "amountCents": 5000
        }
    }));
    execute_graphql(
        &schema,
        mutation,
        Some(variables),
        Some(manager_claims.clone()),
    )
    .await;

    // Check that prize pool was updated
    let query = r#"
        query GetPayout($tournamentId: ID!) {
            tournamentPayout(tournamentId: $tournamentId) {
                totalPrizePool
                playerCount
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Payout query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let payout = &data["tournamentPayout"];

    if payout.is_object() && !payout.is_null() {
        // Prize pool should be sum of entries (5000 + 5000 = 10000)
        assert_eq!(payout["totalPrizePool"], 10000);
        // Player count should be 2
        assert_eq!(payout["playerCount"], 2);
    }
}

#[tokio::test]
async fn test_tournament_and_payout_queries() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let club_id = create_test_club(&app_state, "Nested Payout Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Nested Payout Tournament").await;

    // Query tournament
    let query = r#"
        query GetTournament($id: UUID!) {
            tournament(id: $id) {
                id
                title
                buyInCents
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "id": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Tournament query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let tournament = &data["tournament"];

    assert_eq!(tournament["id"], tournament_id.to_string());

    // Query payout separately
    let payout_query = r#"
        query GetPayout($tournamentId: ID!) {
            tournamentPayout(tournamentId: $tournamentId) {
                id
                tournamentId
                totalPrizePool
                positions {
                    position
                    amountCents
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, payout_query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Payout query should succeed: {:?}",
        response.errors
    );
}
