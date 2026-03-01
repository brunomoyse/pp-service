

use api::gql::build_schema;
use async_graphql::Variables;
use crate::common::*;
use serde_json::json;

#[tokio::test]
async fn test_add_tournament_entry_initial() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    // Create manager and club
    let (manager_id, manager_claims) =
        create_test_user(&app_state, "entry_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Entry Test Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    // Create tournament
    let tournament_id = create_test_tournament(&app_state, club_id, "Entry Test Tournament").await;

    // Create a player
    let (player_id, _) = create_test_user(&app_state, "entry_player@test.com", "player").await;

    // Add initial entry
    let mutation = r#"
        mutation AddEntry($input: AddTournamentEntryInput!) {
            addTournamentEntry(input: $input) {
                id
                tournamentId
                userId
                entryType
                amountCents
                chipsReceived
                notes
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string(),
            "entryType": "INITIAL",
            "amountCents": 5000,
            "chipsReceived": 10000,
            "notes": "Initial buy-in"
        }
    }));

    let response = execute_graphql(&schema, mutation, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Add entry mutation should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let entry = &data["addTournamentEntry"];

    assert_eq!(entry["tournamentId"], tournament_id.to_string());
    assert_eq!(entry["userId"], player_id.to_string());
    assert_eq!(entry["entryType"], "INITIAL");
    assert_eq!(entry["amountCents"], 5000);
    assert_eq!(entry["chipsReceived"], 10000);
    assert_eq!(entry["notes"], "Initial buy-in");
}

#[tokio::test]
async fn test_add_tournament_entry_rebuy() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "rebuy_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Rebuy Test Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let tournament_id = create_test_tournament(&app_state, club_id, "Rebuy Test Tournament").await;
    let (player_id, _) = create_test_user(&app_state, "rebuy_player@test.com", "player").await;

    // Add rebuy entry
    let mutation = r#"
        mutation AddEntry($input: AddTournamentEntryInput!) {
            addTournamentEntry(input: $input) {
                id
                entryType
                amountCents
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string(),
            "entryType": "REBUY",
            "amountCents": 2500
        }
    }));

    let response = execute_graphql(&schema, mutation, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Rebuy entry should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["addTournamentEntry"]["entryType"], "REBUY");
    assert_eq!(data["addTournamentEntry"]["amountCents"], 2500);
}

#[tokio::test]
async fn test_add_tournament_entry_addon() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "addon_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Addon Test Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let tournament_id = create_test_tournament(&app_state, club_id, "Addon Test Tournament").await;
    let (player_id, _) = create_test_user(&app_state, "addon_player@test.com", "player").await;

    let mutation = r#"
        mutation AddEntry($input: AddTournamentEntryInput!) {
            addTournamentEntry(input: $input) {
                entryType
                chipsReceived
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string(),
            "entryType": "ADDON",
            "amountCents": 1000,
            "chipsReceived": 5000
        }
    }));

    let response = execute_graphql(&schema, mutation, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Addon entry should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["addTournamentEntry"]["entryType"], "ADDON");
    assert_eq!(data["addTournamentEntry"]["chipsReceived"], 5000);
}

#[tokio::test]
async fn test_add_tournament_entry_unauthorized() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    // Create a player (not a manager)
    let (player_id, player_claims) =
        create_test_user(&app_state, "unauthorized_entry_player@test.com", "player").await;

    let club_id = create_test_club(&app_state, "Unauthorized Entry Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Unauthorized Entry Tournament").await;

    let mutation = r#"
        mutation AddEntry($input: AddTournamentEntryInput!) {
            addTournamentEntry(input: $input) {
                id
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string(),
            "entryType": "INITIAL",
            "amountCents": 5000
        }
    }));

    let response = execute_graphql(&schema, mutation, Some(variables), Some(player_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Non-manager should not be able to add entries"
    );
}

#[tokio::test]
async fn test_get_tournament_entries() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "get_entries_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Get Entries Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let tournament_id = create_test_tournament(&app_state, club_id, "Get Entries Tournament").await;
    let (player_id, _) =
        create_test_user(&app_state, "get_entries_player@test.com", "player").await;

    // Add an entry first
    let mutation = r#"
        mutation AddEntry($input: AddTournamentEntryInput!) {
            addTournamentEntry(input: $input) {
                id
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string(),
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

    // Query entries
    let query = r#"
        query GetEntries($tournamentId: ID!) {
            tournamentEntries(tournamentId: $tournamentId) {
                id
                tournamentId
                userId
                entryType
                amountCents
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Get entries query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let entries = data["tournamentEntries"].as_array().unwrap();

    assert!(!entries.is_empty(), "Should have at least one entry");
    assert_eq!(entries[0]["entryType"], "INITIAL");
}

#[tokio::test]
async fn test_get_tournament_entry_stats() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "stats_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Stats Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let tournament_id = create_test_tournament(&app_state, club_id, "Stats Tournament").await;
    let (player1_id, _) = create_test_user(&app_state, "stats_player1@test.com", "player").await;
    let (player2_id, _) = create_test_user(&app_state, "stats_player2@test.com", "player").await;

    // Add entries for two players
    let mutation = r#"
        mutation AddEntry($input: AddTournamentEntryInput!) {
            addTournamentEntry(input: $input) {
                id
            }
        }
    "#;

    // Player 1: initial
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

    // Player 1: rebuy
    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player1_id.to_string(),
            "entryType": "REBUY",
            "amountCents": 2500
        }
    }));
    execute_graphql(
        &schema,
        mutation,
        Some(variables),
        Some(manager_claims.clone()),
    )
    .await;

    // Player 2: initial
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

    // Query stats
    let query = r#"
        query GetStats($tournamentId: ID!) {
            tournamentEntryStats(tournamentId: $tournamentId) {
                tournamentId
                totalEntries
                totalAmountCents
                uniquePlayers
                initialCount
                rebuyCount
                reEntryCount
                addonCount
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Get stats query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let stats = &data["tournamentEntryStats"];

    assert_eq!(stats["totalEntries"], 3);
    assert_eq!(stats["totalAmountCents"], 12500); // 5000 + 2500 + 5000
    assert_eq!(stats["uniquePlayers"], 2);
    assert_eq!(stats["initialCount"], 2);
    assert_eq!(stats["rebuyCount"], 1);
    assert_eq!(stats["reEntryCount"], 0);
    assert_eq!(stats["addonCount"], 0);
}

#[tokio::test]
async fn test_delete_tournament_entry() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "delete_entry_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Delete Entry Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let tournament_id =
        create_test_tournament(&app_state, club_id, "Delete Entry Tournament").await;
    let (player_id, _) =
        create_test_user(&app_state, "delete_entry_player@test.com", "player").await;

    // Add an entry
    let mutation = r#"
        mutation AddEntry($input: AddTournamentEntryInput!) {
            addTournamentEntry(input: $input) {
                id
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string(),
            "entryType": "INITIAL",
            "amountCents": 5000
        }
    }));

    let response = execute_graphql(
        &schema,
        mutation,
        Some(variables),
        Some(manager_claims.clone()),
    )
    .await;
    let data = response.data.into_json().unwrap();
    let entry_id = data["addTournamentEntry"]["id"].as_str().unwrap();

    // Delete the entry
    let delete_mutation = r#"
        mutation DeleteEntry($entryId: ID!) {
            deleteTournamentEntry(entryId: $entryId)
        }
    "#;

    let variables = Variables::from_json(json!({
        "entryId": entry_id
    }));

    let response = execute_graphql(
        &schema,
        delete_mutation,
        Some(variables),
        Some(manager_claims),
    )
    .await;

    assert!(
        response.errors.is_empty(),
        "Delete entry mutation should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["deleteTournamentEntry"], true);
}
