mod common;

use api::gql::build_schema;
use async_graphql::Variables;
use common::*;
use serde_json::json;

/// Helper: create tournament structures and clock for a tournament
async fn setup_clock(
    app_state: &api::AppState,
    schema: &async_graphql::Schema<
        api::gql::QueryRoot,
        api::gql::MutationRoot,
        api::gql::SubscriptionRoot,
    >,
    tournament_id: uuid::Uuid,
    manager_claims: &api::auth::Claims,
) {
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
        schema,
        create_query,
        Some(variables),
        Some(manager_claims.clone()),
    )
    .await;

    assert!(
        response.errors.is_empty() || response.errors[0].message.contains("duplicate key"),
        "Create clock should succeed or already exist: {:?}",
        response.errors
    );
}

#[tokio::test]
async fn test_start_tournament_clock() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("startclock_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Start Clock Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Start Clock Tournament").await;

    setup_clock(&app_state, &schema, tournament_id, &manager_claims).await;

    let query = r#"
        mutation StartTournamentClock($tournamentId: ID!) {
            startTournamentClock(tournamentId: $tournamentId) {
                id
                status
                currentLevel
                timeRemainingSeconds
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Start clock should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let clock = &data["startTournamentClock"];

    assert_eq!(clock["status"], "RUNNING");
    assert!(
        clock["timeRemainingSeconds"].as_i64().is_some(),
        "timeRemainingSeconds should be populated"
    );
}

#[tokio::test]
async fn test_pause_tournament_clock() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("pauseclock_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Pause Clock Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Pause Clock Tournament").await;

    setup_clock(&app_state, &schema, tournament_id, &manager_claims).await;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    // Start the clock first
    let start_query = r#"
        mutation StartTournamentClock($tournamentId: ID!) {
            startTournamentClock(tournamentId: $tournamentId) { id status }
        }
    "#;
    let response = execute_graphql(
        &schema,
        start_query,
        Some(variables.clone()),
        Some(manager_claims.clone()),
    )
    .await;
    assert!(
        response.errors.is_empty(),
        "Start should succeed: {:?}",
        response.errors
    );

    // Now pause
    let pause_query = r#"
        mutation PauseTournamentClock($tournamentId: ID!) {
            pauseTournamentClock(tournamentId: $tournamentId) {
                id
                status
            }
        }
    "#;

    let response =
        execute_graphql(&schema, pause_query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Pause clock should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["pauseTournamentClock"]["status"], "PAUSED");
}

#[tokio::test]
async fn test_resume_tournament_clock() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("resumeclock_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Resume Clock Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Resume Clock Tournament").await;

    setup_clock(&app_state, &schema, tournament_id, &manager_claims).await;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    // Start then pause
    let start_query = r#"
        mutation StartTournamentClock($tournamentId: ID!) {
            startTournamentClock(tournamentId: $tournamentId) { id }
        }
    "#;
    execute_graphql(
        &schema,
        start_query,
        Some(variables.clone()),
        Some(manager_claims.clone()),
    )
    .await;

    let pause_query = r#"
        mutation PauseTournamentClock($tournamentId: ID!) {
            pauseTournamentClock(tournamentId: $tournamentId) { id }
        }
    "#;
    execute_graphql(
        &schema,
        pause_query,
        Some(variables.clone()),
        Some(manager_claims.clone()),
    )
    .await;

    // Now resume
    let resume_query = r#"
        mutation ResumeTournamentClock($tournamentId: ID!) {
            resumeTournamentClock(tournamentId: $tournamentId) {
                id
                status
            }
        }
    "#;

    let response =
        execute_graphql(&schema, resume_query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Resume clock should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["resumeTournamentClock"]["status"], "RUNNING");
}

#[tokio::test]
async fn test_clock_full_lifecycle() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("lifecycle_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Lifecycle Clock Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Lifecycle Tournament").await;

    setup_clock(&app_state, &schema, tournament_id, &manager_claims).await;

    let variables = Variables::from_json(json!({
        "tournamentId": tournament_id.to_string()
    }));

    // 1. Start → RUNNING
    let start_query = r#"
        mutation StartTournamentClock($tournamentId: ID!) {
            startTournamentClock(tournamentId: $tournamentId) { status currentLevel }
        }
    "#;
    let response = execute_graphql(
        &schema,
        start_query,
        Some(variables.clone()),
        Some(manager_claims.clone()),
    )
    .await;
    assert!(response.errors.is_empty(), "Start: {:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["startTournamentClock"]["status"], "RUNNING");
    assert_eq!(data["startTournamentClock"]["currentLevel"], 1);

    // 2. Pause → PAUSED
    let pause_query = r#"
        mutation PauseTournamentClock($tournamentId: ID!) {
            pauseTournamentClock(tournamentId: $tournamentId) { status }
        }
    "#;
    let response = execute_graphql(
        &schema,
        pause_query,
        Some(variables.clone()),
        Some(manager_claims.clone()),
    )
    .await;
    assert!(response.errors.is_empty(), "Pause: {:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["pauseTournamentClock"]["status"], "PAUSED");

    // 3. Resume → RUNNING
    let resume_query = r#"
        mutation ResumeTournamentClock($tournamentId: ID!) {
            resumeTournamentClock(tournamentId: $tournamentId) { status }
        }
    "#;
    let response = execute_graphql(
        &schema,
        resume_query,
        Some(variables.clone()),
        Some(manager_claims.clone()),
    )
    .await;
    assert!(response.errors.is_empty(), "Resume: {:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["resumeTournamentClock"]["status"], "RUNNING");

    // 4. Advance level → level 2
    let advance_query = r#"
        mutation AdvanceTournamentLevel($tournamentId: ID!) {
            advanceTournamentLevel(tournamentId: $tournamentId) { currentLevel status }
        }
    "#;
    let response = execute_graphql(
        &schema,
        advance_query,
        Some(variables),
        Some(manager_claims),
    )
    .await;
    assert!(response.errors.is_empty(), "Advance: {:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["advanceTournamentLevel"]["currentLevel"], 2);
}

#[tokio::test]
async fn test_start_clock_unauthorized() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (_, player_claims) = create_test_user(
        &app_state,
        &format!("clockplayer_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Unauth Clock Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Unauth Clock Tournament").await;

    let query = r#"
        mutation StartTournamentClock($tournamentId: ID!) {
            startTournamentClock(tournamentId: $tournamentId) {
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
        "Player should not be able to start clock"
    );
    assert!(
        response.errors[0]
            .message
            .contains("Manager privileges required"),
        "Expected manager privileges error, got: '{}'",
        response.errors[0].message
    );
}
