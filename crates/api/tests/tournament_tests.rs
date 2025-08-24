mod common;

use api::gql::build_schema;
use async_graphql::Variables;
use common::*;
use serde_json::json;

#[tokio::test]
async fn test_get_tournaments_query() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let club_id = create_test_club(&app_state, "Tournament Club").await;
    let _tournament_id = create_test_tournament(&app_state, club_id, "Test Tournament").await;

    let query = r#"
        query GetTournaments($limit: Int, $offset: Int) {
            tournaments(limit: $limit, offset: $offset) {
                id
                title
                status
                liveStatus
                buyInCents
                seatCap
                club {
                    id
                    name
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "limit": 10,
        "offset": 0
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Tournaments query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let tournaments = data["tournaments"].as_array().unwrap();

    assert!(
        !tournaments.is_empty(),
        "Should return at least one tournament"
    );
}

#[tokio::test]
async fn test_get_tournament_by_id() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let club_id = create_test_club(&app_state, "Single Tournament Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Specific Tournament").await;

    // Test the new singular tournament query
    let query = r#"
        query GetTournament($id: UUID!) {
            tournament(id: $id) {
                id
                title
                description
                status
                liveStatus
                buyInCents
                seatCap
                clubId
                startTime
                endTime
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
    assert_eq!(tournament["title"], "Specific Tournament");
    assert_eq!(tournament["description"], "Test tournament description");
    assert_eq!(tournament["buyInCents"], 5000);
    assert_eq!(tournament["seatCap"], 100);
    assert_eq!(tournament["clubId"], club_id.to_string());
}

#[tokio::test]
async fn test_get_nonexistent_tournament() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let nonexistent_id = uuid::Uuid::new_v4();

    let query = r#"
        query GetTournament($id: UUID!) {
            tournament(id: $id) {
                id
                title
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "id": nonexistent_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Query should not error for nonexistent tournament: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert!(
        data["tournament"].is_null(),
        "Should return null for nonexistent tournament"
    );
}

#[tokio::test]
async fn test_register_for_tournament() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (user_id, claims) = create_test_user(&app_state, "playerreg@test.com", "player").await;
    let club_id = create_test_club(&app_state, "Registration Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Registration Tournament").await;

    let query = r#"
        mutation RegisterForTournament($input: RegisterForTournamentInput!) {
            registerForTournament(input: $input) {
                id
                tournamentId
                userId
                registrationTime
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string()
            // Don't provide userId - let the player register themselves
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(claims)).await;

    assert!(
        response.errors.is_empty(),
        "Tournament registration should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let registration = &data["registerForTournament"];

    assert_eq!(registration["tournamentId"], tournament_id.to_string());
    assert_eq!(registration["userId"], user_id.to_string());
}

#[tokio::test]
async fn test_tournament_with_nested_fields() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let club_id = create_test_club(&app_state, "Nested Fields Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Tournament With Nested Data").await;

    // Create some test data for nested fields
    // 1. Add some tournament registrations
    let (user1_id, _) = create_test_user(&app_state, "user1@test.com", "player").await;
    let (user2_id, _) = create_test_user(&app_state, "user2@test.com", "player").await;

    // Register users for the tournament
    for user_id in [user1_id, user2_id] {
        sqlx::query!(
            r#"INSERT INTO tournament_registrations (tournament_id, user_id, status) 
               VALUES ($1, $2, 'registered') 
               ON CONFLICT DO NOTHING"#,
            tournament_id,
            user_id
        )
        .execute(&app_state.db)
        .await
        .expect("Failed to register user for tournament");
    }

    // 2. Add some tournament structure levels (first remove any existing ones to avoid unique constraint violations)
    sqlx::query!(
        "DELETE FROM tournament_structures WHERE tournament_id = $1",
        tournament_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to clean existing tournament structures");

    sqlx::query!(
        r#"INSERT INTO tournament_structures (tournament_id, level_number, small_blind, big_blind, ante, duration_minutes, is_break) 
           VALUES ($1, 1, 25, 50, 0, 20, false), 
                  ($1, 2, 50, 100, 0, 20, false),
                  ($1, 3, 0, 0, 0, 15, true)"#,
        tournament_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create tournament structure");

    // 3. Create a tournament clock
    sqlx::query!(
        r#"INSERT INTO tournament_clocks (tournament_id, clock_status, current_level, auto_advance) 
           VALUES ($1, 'stopped', 1, true) 
           ON CONFLICT DO NOTHING"#,
        tournament_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to create tournament clock");

    // Test the nested fields query
    let query = r#"
        query GetTournamentWithNestedFields($id: UUID!) {
            tournament(id: $id) {
                id
                title
                structure {
                    id
                    levelNumber
                    smallBlind
                    bigBlind
                    ante
                    durationMinutes
                    isBreak
                }
                clock {
                    id
                    status
                    currentLevel
                    autoAdvance
                }
                registrations {
                    id
                    userId
                    status
                    user {
                        id
                        email
                        firstName
                        lastName
                        role
                    }
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "id": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Tournament with nested fields query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let tournament = &data["tournament"];

    // Verify tournament data
    assert_eq!(tournament["id"], tournament_id.to_string());
    assert_eq!(tournament["title"], "Tournament With Nested Data");

    // Verify tournament structure
    let structures = tournament["structure"].as_array().unwrap();
    assert_eq!(
        structures.len(),
        3,
        "Should have 3 tournament structure levels"
    );

    // Check first level
    assert_eq!(structures[0]["levelNumber"], 1);
    assert_eq!(structures[0]["smallBlind"], 25);
    assert_eq!(structures[0]["bigBlind"], 50);
    assert_eq!(structures[0]["ante"], 0);
    assert_eq!(structures[0]["durationMinutes"], 20);
    assert_eq!(structures[0]["isBreak"], false);

    // Check break level
    assert_eq!(structures[2]["levelNumber"], 3);
    assert_eq!(structures[2]["isBreak"], true);

    // Verify tournament clock
    let clock = &tournament["clock"];
    assert!(!clock.is_null(), "Tournament clock should exist");
    assert_eq!(clock["status"], "STOPPED");
    assert_eq!(clock["currentLevel"], 1);
    assert_eq!(clock["autoAdvance"], true);

    // Verify tournament registrations
    let registrations = tournament["registrations"].as_array().unwrap();
    assert_eq!(registrations.len(), 2, "Should have 2 registrations");

    for registration in registrations {
        assert_eq!(registration["status"], "REGISTERED");
        assert!(
            registration["userId"] == user1_id.to_string()
                || registration["userId"] == user2_id.to_string()
        );

        // Verify user object is populated
        let user = &registration["user"];
        assert!(!user.is_null(), "User should be populated");
        assert!(user["email"].is_string(), "User email should be present");
        assert!(
            user["firstName"].is_string(),
            "User firstName should be present"
        );
        assert_eq!(user["role"], "PLAYER");
        assert!(user["id"] == user1_id.to_string() || user["id"] == user2_id.to_string());
    }
}

#[tokio::test]
async fn test_tournament_registration_user_field() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let club_id = create_test_club(&app_state, "Registration User Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "User Registration Tournament").await;

    // Create a test user and register them
    let (user_id, _claims) = create_test_user(&app_state, "testuser@test.com", "player").await;

    // Register the user for the tournament
    sqlx::query!(
        r#"INSERT INTO tournament_registrations (tournament_id, user_id, status) 
           VALUES ($1, $2, 'registered') 
           ON CONFLICT DO NOTHING"#,
        tournament_id,
        user_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to register user for tournament");

    // Query just the registrations with user data
    let query = r#"
        query GetTournamentRegistrations($id: UUID!) {
            tournament(id: $id) {
                registrations {
                    id
                    userId
                    status
                    user {
                        id
                        email
                        firstName
                        lastName
                        role
                        isActive
                        managedClub {
                            id
                            name
                            city
                        }
                    }
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "id": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let registrations = data["tournament"]["registrations"].as_array().unwrap();

    assert_eq!(registrations.len(), 1, "Should have 1 registration");

    let registration = &registrations[0];
    assert_eq!(registration["status"], "REGISTERED");
    assert_eq!(registration["userId"], user_id.to_string());

    let user = &registration["user"];
    assert!(!user.is_null(), "User should be populated");
    assert_eq!(user["id"], user_id.to_string());
    assert_eq!(user["email"], "testuser@test.com");
    assert_eq!(user["role"], "PLAYER");
    assert_eq!(user["isActive"], true);
    assert!(user["firstName"].is_string(), "firstName should be present");
}

#[tokio::test]
async fn test_user_managed_club_field() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    // Create a test club first
    let club_id = create_test_club(&app_state, "Manager Test Club").await;

    // Create a manager user and assign them to the club
    let (manager_id, _claims) = create_test_user(&app_state, "manager@test.com", "manager").await;

    // Assign the manager to the club
    sqlx::query!(
        r#"INSERT INTO club_managers (club_id, user_id, assigned_by, notes) 
           VALUES ($1, $2, NULL, 'Test manager assignment')"#,
        club_id,
        manager_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to assign manager to club");

    // Create a tournament so we can query registrations
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Manager Test Tournament").await;

    // Register the manager for the tournament
    sqlx::query!(
        r#"INSERT INTO tournament_registrations (tournament_id, user_id, status) 
           VALUES ($1, $2, 'registered')"#,
        tournament_id,
        manager_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to register manager for tournament");

    // Query the user with managedClub field
    let query = r#"
        query GetTournamentWithManager($id: UUID!) {
            tournament(id: $id) {
                registrations {
                    user {
                        id
                        email
                        role
                        managedClub {
                            id
                            name
                            city
                        }
                    }
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "id": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let registrations = data["tournament"]["registrations"].as_array().unwrap();

    assert_eq!(registrations.len(), 1, "Should have 1 registration");

    let registration = &registrations[0];
    let user = &registration["user"];

    assert_eq!(user["id"], manager_id.to_string());
    assert_eq!(user["email"], "manager@test.com");
    assert_eq!(user["role"], "MANAGER");

    // Verify managedClub is populated for the manager
    let managed_club = &user["managedClub"];
    assert!(!managed_club.is_null(), "Manager should have a managedClub");

    // The club ID should be valid UUID and name should match
    let managed_club_id = managed_club["id"]
        .as_str()
        .expect("Club ID should be a string");
    let managed_club_name = managed_club["name"]
        .as_str()
        .expect("Club name should be a string");

    // Verify the UUID is valid
    assert!(
        uuid::Uuid::parse_str(managed_club_id).is_ok(),
        "Club ID should be valid UUID"
    );
    assert_eq!(managed_club_name, "Manager Test Club");
}

#[tokio::test]
async fn test_regular_player_has_no_managed_club() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let club_id = create_test_club(&app_state, "Regular Player Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Regular Player Tournament").await;

    // Create a regular player (not a manager)
    let (player_id, _claims) =
        create_test_user(&app_state, "regularplayer@test.com", "player").await;

    // Register the player for the tournament
    sqlx::query!(
        r#"INSERT INTO tournament_registrations (tournament_id, user_id, status) 
           VALUES ($1, $2, 'registered')"#,
        tournament_id,
        player_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to register player for tournament");

    let query = r#"
        query GetTournamentWithRegularPlayer($id: UUID!) {
            tournament(id: $id) {
                registrations {
                    user {
                        id
                        role
                        managedClub {
                            id
                            name
                        }
                    }
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "id": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let registrations = data["tournament"]["registrations"].as_array().unwrap();

    assert_eq!(registrations.len(), 1, "Should have 1 registration");

    let registration = &registrations[0];
    let user = &registration["user"];

    assert_eq!(user["role"], "PLAYER");

    // Verify managedClub is null for regular players
    let managed_club = &user["managedClub"];
    assert!(
        managed_club.is_null(),
        "Regular player should not have a managedClub"
    );
}
