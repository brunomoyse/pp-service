use crate::common::*;
use api::gql::build_schema;
use async_graphql::Variables;
use serde_json::json;

#[tokio::test]
async fn test_enter_tournament_results() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    // Create manager and club
    let (manager_id, manager_claims) =
        create_test_user(&app_state, "results_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Results Test Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    // Create tournament
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Results Test Tournament").await;

    // Create players
    let (player1_id, _) = create_test_user(&app_state, "results_player1@test.com", "player").await;
    let (player2_id, _) = create_test_user(&app_state, "results_player2@test.com", "player").await;
    let (player3_id, _) = create_test_user(&app_state, "results_player3@test.com", "player").await;

    // Enter results
    let mutation = r#"
        mutation EnterResults($input: EnterTournamentResultsInput!) {
            enterTournamentResults(input: $input) {
                success
                results {
                    id
                    userId
                    tournamentId
                    finalPosition
                    prizeCents
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "playerPositions": [
                { "userId": player1_id.to_string(), "finalPosition": 1 },
                { "userId": player2_id.to_string(), "finalPosition": 2 },
                { "userId": player3_id.to_string(), "finalPosition": 3 }
            ]
        }
    }));

    let response = execute_graphql(&schema, mutation, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Enter results should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["enterTournamentResults"]["success"], true);

    let results = data["enterTournamentResults"]["results"]
        .as_array()
        .unwrap();
    assert_eq!(results.len(), 3, "Should have 3 results");

    // Verify positions are correct
    let positions: Vec<i64> = results
        .iter()
        .map(|r| r["finalPosition"].as_i64().unwrap())
        .collect();
    assert!(positions.contains(&1));
    assert!(positions.contains(&2));
    assert!(positions.contains(&3));
}

#[tokio::test]
async fn test_enter_tournament_results_unauthorized() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    // Create a player (not manager)
    let (player_id, player_claims) =
        create_test_user(&app_state, "unauth_results_player@test.com", "player").await;

    let club_id = create_test_club(&app_state, "Unauthorized Results Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Unauthorized Results Tournament").await;

    let mutation = r#"
        mutation EnterResults($input: EnterTournamentResultsInput!) {
            enterTournamentResults(input: $input) {
                success
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "playerPositions": [
                { "userId": player_id.to_string(), "finalPosition": 1 }
            ]
        }
    }));

    let response = execute_graphql(&schema, mutation, Some(variables), Some(player_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Non-manager should not be able to enter results"
    );
}

#[tokio::test]
async fn test_leaderboard_query() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    // Create test data: manager, club, tournaments, and results
    let (manager_id, manager_claims) =
        create_test_user(&app_state, "leaderboard_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Leaderboard Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let tournament_id = create_test_tournament(&app_state, club_id, "Leaderboard Tournament").await;

    // Create players and enter results
    let (player1_id, _) =
        create_test_user(&app_state, "leaderboard_player1@test.com", "player").await;
    let (player2_id, _) =
        create_test_user(&app_state, "leaderboard_player2@test.com", "player").await;

    // Enter results to populate leaderboard
    let mutation = r#"
        mutation EnterResults($input: EnterTournamentResultsInput!) {
            enterTournamentResults(input: $input) {
                success
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "playerPositions": [
                { "userId": player1_id.to_string(), "finalPosition": 1 },
                { "userId": player2_id.to_string(), "finalPosition": 2 }
            ]
        }
    }));

    execute_graphql(&schema, mutation, Some(variables), Some(manager_claims)).await;

    // Query leaderboard
    let query = r#"
        query GetLeaderboard($clubId: ID, $period: LeaderboardPeriod, $pagination: PaginationInput) {
            leaderboard(clubId: $clubId, period: $period, pagination: $pagination) {
                items {
                    user {
                        id
                        email
                    }
                    rank
                    totalWinnings
                    totalTournaments
                    firstPlaces
                }
                totalCount
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "clubId": club_id.to_string(),
        "period": "LAST_30_DAYS",
        "pagination": { "limit": 10, "offset": 0 }
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Leaderboard query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let leaderboard = &data["leaderboard"];

    // Leaderboard response structure should be valid
    assert!(leaderboard["items"].is_array(), "Items should be an array");
}

#[tokio::test]
async fn test_my_tournament_statistics() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    // Create a player with some tournament history
    let (player_id, player_claims) =
        create_test_user(&app_state, "stats_test_player@test.com", "player").await;

    // Create manager and tournament
    let (manager_id, manager_claims) =
        create_test_user(&app_state, "stats_test_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Stats Test Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let tournament_id = create_test_tournament(&app_state, club_id, "Stats Tournament").await;

    // Enter a result for the player
    let mutation = r#"
        mutation EnterResults($input: EnterTournamentResultsInput!) {
            enterTournamentResults(input: $input) {
                success
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "playerPositions": [
                { "userId": player_id.to_string(), "finalPosition": 1 }
            ]
        }
    }));

    execute_graphql(&schema, mutation, Some(variables), Some(manager_claims)).await;

    // Query player's statistics
    let query = r#"
        query MyStatistics {
            myTournamentStatistics {
                last7Days {
                    totalItm
                    totalTournaments
                    totalWinnings
                    totalBuyIns
                    itmPercentage
                    roiPercentage
                }
                last30Days {
                    totalItm
                    totalTournaments
                }
                lastYear {
                    totalItm
                    totalTournaments
                }
            }
        }
    "#;

    let response = execute_graphql(&schema, query, None, Some(player_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Statistics query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let stats = &data["myTournamentStatistics"];

    // Verify structure exists
    assert!(stats["last7Days"].is_object());
    assert!(stats["last30Days"].is_object());
    assert!(stats["lastYear"].is_object());
}

#[tokio::test]
async fn test_my_recent_tournament_results() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (player_id, player_claims) =
        create_test_user(&app_state, "recent_results_player@test.com", "player").await;

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "recent_results_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Recent Results Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let tournament_id =
        create_test_tournament(&app_state, club_id, "Recent Results Tournament").await;

    // Enter a result
    let mutation = r#"
        mutation EnterResults($input: EnterTournamentResultsInput!) {
            enterTournamentResults(input: $input) {
                success
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "playerPositions": [
                { "userId": player_id.to_string(), "finalPosition": 2 }
            ]
        }
    }));

    execute_graphql(&schema, mutation, Some(variables), Some(manager_claims)).await;

    // Query recent results
    let query = r#"
        query MyRecentResults($limit: Int) {
            myRecentTournamentResults(limit: $limit) {
                result {
                    id
                    tournamentId
                    finalPosition
                    prizeCents
                }
                tournament {
                    id
                    title
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "limit": 10
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(player_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Recent results query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let results = data["myRecentTournamentResults"].as_array().unwrap();

    assert!(
        !results.is_empty(),
        "Should have at least one recent result"
    );
    assert_eq!(results[0]["result"]["finalPosition"], 2);
}
