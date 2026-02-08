mod common;

use api::gql::build_schema;
use async_graphql::Variables;
use common::*;
use serde_json::json;

#[tokio::test]
async fn test_my_tournament_registrations() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (player_id, player_claims) = create_test_user(
        &app_state,
        &format!("myreg_player_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "My Reg Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "My Reg Tournament").await;

    // Register the player
    create_test_registration(&app_state, tournament_id, player_id, "registered").await;

    let query = r#"
        query MyTournamentRegistrations {
            myTournamentRegistrations {
                id
                tournamentId
                userId
                status
            }
        }
    "#;

    let response = execute_graphql(&schema, query, None, Some(player_claims)).await;

    assert!(
        response.errors.is_empty(),
        "myTournamentRegistrations should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let registrations = data["myTournamentRegistrations"].as_array().unwrap();

    assert!(
        !registrations.is_empty(),
        "Should return at least one registration"
    );

    // Find our registration
    let found = registrations.iter().any(|r| {
        r["userId"] == player_id.to_string() && r["tournamentId"] == tournament_id.to_string()
    });
    assert!(found, "Should find the player's registration");
}

#[tokio::test]
async fn test_my_tournament_registrations_unauthenticated() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let query = r#"
        query MyTournamentRegistrations {
            myTournamentRegistrations {
                id
            }
        }
    "#;

    let response = execute_graphql(&schema, query, None, None).await;

    assert!(
        !response.errors.is_empty(),
        "Unauthenticated myTournamentRegistrations should fail"
    );
}

#[tokio::test]
async fn test_tournament_tables_query() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("ttables_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Tables Query Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Tables Query Tournament").await;

    // Create and assign two tables
    let table1_id = create_test_club_table(&app_state, club_id, 1, 9).await;
    let table2_id = create_test_club_table(&app_state, club_id, 2, 6).await;
    assign_table_to_tournament(&app_state, tournament_id, table1_id).await;
    assign_table_to_tournament(&app_state, tournament_id, table2_id).await;

    let query = r#"
        query TournamentTables($tournamentId: ID!) {
            tournamentTables(tournamentId: $tournamentId) {
                id
                tournamentId
                tableNumber
                maxSeats
                isActive
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "tournamentTables should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let tables = data["tournamentTables"].as_array().unwrap();

    assert_eq!(tables.len(), 2, "Should return 2 assigned tables");
}

#[tokio::test]
async fn test_tournament_seating_chart() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("seatchart_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Seating Chart Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Seating Chart Tournament").await;

    // Create a table and assign it
    let table_id = create_test_club_table(&app_state, club_id, 1, 9).await;
    assign_table_to_tournament(&app_state, tournament_id, table_id).await;

    let query = r#"
        query TournamentSeatingChart($tournamentId: ID!) {
            tournamentSeatingChart(tournamentId: $tournamentId) {
                tournament {
                    id
                    title
                }
                tables {
                    table {
                        id
                        tableNumber
                        maxSeats
                    }
                    seats {
                        assignment {
                            seatNumber
                        }
                        player {
                            id
                        }
                    }
                }
                unassignedPlayers {
                    id
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "tournamentSeatingChart should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let chart = &data["tournamentSeatingChart"];

    assert_eq!(chart["tournament"]["id"], tournament_id.to_string());
    assert!(
        !chart["tables"].as_array().unwrap().is_empty(),
        "Should have at least 1 table"
    );
}

#[tokio::test]
async fn test_tournament_seating_history() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("seathistory_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let (player_id, _) = create_test_user(
        &app_state,
        &format!("seathistory_player_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Seat History Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Seat History Tournament").await;

    // Create two tables and assign them
    let table1_id = create_test_club_table(&app_state, club_id, 1, 9).await;
    let table2_id = create_test_club_table(&app_state, club_id, 2, 9).await;
    assign_table_to_tournament(&app_state, tournament_id, table1_id).await;
    assign_table_to_tournament(&app_state, tournament_id, table2_id).await;

    // Seat the player at table 1
    sqlx::query!(
        "INSERT INTO table_seat_assignments (tournament_id, club_table_id, user_id, seat_number, stack_size) VALUES ($1, $2, $3, $4, $5)",
        tournament_id,
        table1_id,
        player_id,
        1,
        20000
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to seat player");

    // Move the player to table 2 via mutation
    let move_query = r#"
        mutation MovePlayer($input: MovePlayerInput!) {
            movePlayer(input: $input) {
                id
                seatNumber
                clubTableId
            }
        }
    "#;

    let move_variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string(),
            "newClubTableId": table2_id.to_string(),
            "newSeatNumber": 3
        }
    }));

    let response = execute_graphql(
        &schema,
        move_query,
        Some(move_variables),
        Some(manager_claims.clone()),
    )
    .await;
    assert!(
        response.errors.is_empty(),
        "Move player should succeed: {:?}",
        response.errors
    );

    // Now query seating history
    let history_query = r#"
        query TournamentSeatingHistory($tournamentId: ID!, $limit: Int) {
            tournamentSeatingHistory(tournamentId: $tournamentId, limit: $limit) {
                id
                userId
                clubTableId
                seatNumber
                isCurrent
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string(),
        "limit": 50
    }));

    let response = execute_graphql(
        &schema,
        history_query,
        Some(variables),
        Some(manager_claims),
    )
    .await;

    assert!(
        response.errors.is_empty(),
        "tournamentSeatingHistory should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let history = data["tournamentSeatingHistory"].as_array().unwrap();

    // Should have at least 2 entries: the original assignment (now is_current=false) and the new one (is_current=true)
    assert!(
        history.len() >= 2,
        "Should have at least 2 seating history entries, got {}",
        history.len()
    );

    let current_entries: Vec<_> = history.iter().filter(|h| h["isCurrent"] == true).collect();
    assert_eq!(
        current_entries.len(),
        1,
        "Should have exactly 1 current entry"
    );
    assert_eq!(current_entries[0]["clubTableId"], table2_id.to_string());
}

#[tokio::test]
async fn test_blind_structure_templates() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (_, claims) =
        create_test_user(&app_state, &format!("bst_user_{unique}@test.com"), "admin").await;

    let query = r#"
        query BlindStructureTemplates {
            blindStructureTemplates {
                id
                name
                description
            }
        }
    "#;

    let response = execute_graphql(&schema, query, None, Some(claims)).await;

    assert!(
        response.errors.is_empty(),
        "blindStructureTemplates should not error: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let _templates = data["blindStructureTemplates"].as_array().unwrap();
    // May be empty if no templates seeded, but should not error
}

#[tokio::test]
async fn test_club_tables_via_graphql() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("clubtables_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Club Tables Query Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    // Create some tables
    create_test_club_table(&app_state, club_id, 1, 9).await;
    create_test_club_table(&app_state, club_id, 2, 6).await;

    let query = r#"
        query ClubTables($clubId: ID!) {
            clubTables(clubId: $clubId) {
                id
                clubId
                tableNumber
                maxSeats
                isActive
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "clubId": club_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "clubTables should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let tables = data["clubTables"].as_array().unwrap();

    assert!(tables.len() >= 2, "Should return at least 2 club tables");

    // Check that all tables belong to our club
    for table in tables {
        assert_eq!(table["clubId"], club_id.to_string());
    }
}

#[tokio::test]
async fn test_users_search_filter() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (_, admin_claims) = create_test_user(
        &app_state,
        &format!("searchadmin_{unique}@test.com"),
        "admin",
    )
    .await;

    // Create a player with a distinctive first_name (search filters by username, first_name, last_name)
    let search_email = format!("searchable_{unique}@test.com");
    let (searchable_id, _) = create_test_user(&app_state, &search_email, "player").await;
    // Update the first_name to something unique and searchable
    sqlx::query!(
        "UPDATE users SET first_name = $1 WHERE id = $2",
        format!("Findable{unique}"),
        searchable_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to update first_name");

    // Also deactivate a user to test isActive filter
    let inactive_email = format!("inactive_{unique}@test.com");
    let (inactive_id, _) = create_test_user(&app_state, &inactive_email, "player").await;
    sqlx::query!(
        "UPDATE users SET is_active = false WHERE id = $1",
        inactive_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to deactivate user");

    // Test search filter (searches by first_name, last_name, username)
    let query = r#"
        query Users($search: String, $isActive: Boolean, $limit: Int) {
            users(search: $search, isActive: $isActive, limit: $limit) {
                id
                email
                isActive
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "search": format!("Findable{unique}"),
        "limit": 10
    }));

    let response =
        execute_graphql(&schema, query, Some(variables), Some(admin_claims.clone())).await;

    assert!(
        response.errors.is_empty(),
        "Users search should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let users = data["users"].as_array().unwrap();
    assert!(
        !users.is_empty(),
        "Should find at least one user matching search by first_name"
    );

    // Test isActive filter
    let variables = Variables::from_json(json!({
        "isActive": false,
        "limit": 100
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(admin_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Users isActive filter should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let users = data["users"].as_array().unwrap();

    // All returned users should be inactive
    for user in users {
        assert_eq!(
            user["isActive"], false,
            "All returned users should be inactive"
        );
    }
}
