use crate::common::*;
use api::gql::build_schema;
use async_graphql::Variables;
use serde_json::json;

#[tokio::test]
async fn test_register_for_tournament() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (user_id, claims) = create_test_user(&app_state, "playerreg@test.com", "player").await;
    let club_id = create_test_club(&app_state, "Registration Club").await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, "Registration Tournament").await;

    // Open registration so the player can register
    sqlx::query("UPDATE tournaments SET live_status = 'registration_open'::tournament_live_status WHERE id = $1")
        .bind(tournament_id)
        .execute(&app_state.db)
        .await
        .expect("Failed to open registration");

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
async fn test_create_tournament_with_rake() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "rake_create_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Rake Create Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let mutation = r#"
        mutation CreateTournament($input: CreateTournamentInput!) {
            createTournament(input: $input) {
                id
                title
                buyInCents
                rakeCents
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "clubId": club_id.to_string(),
            "name": "Rake Tournament",
            "startTime": "2026-06-01T18:00:00Z",
            "buyInCents": 5000,
            "rakeCents": 500
        }
    }));

    let response = execute_graphql(
        &schema,
        mutation,
        Some(variables),
        Some(manager_claims.clone()),
    )
    .await;

    assert!(
        response.errors.is_empty(),
        "Create tournament with rake should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let tournament = &data["createTournament"];

    assert_eq!(tournament["title"], "Rake Tournament");
    assert_eq!(tournament["buyInCents"], 5000);
    assert_eq!(tournament["rakeCents"], 500);

    // Verify it persists by querying it back
    let tournament_id = tournament["id"].as_str().unwrap();

    let query = r#"
        query GetTournament($id: ID!) {
            tournament(id: $id) {
                id
                buyInCents
                rakeCents
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "id": tournament_id
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Get tournament should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["tournament"]["buyInCents"], 5000);
    assert_eq!(data["tournament"]["rakeCents"], 500);
}

#[tokio::test]
async fn test_create_tournament_without_rake_defaults_to_zero() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "norake_create_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "No Rake Create Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let mutation = r#"
        mutation CreateTournament($input: CreateTournamentInput!) {
            createTournament(input: $input) {
                id
                buyInCents
                rakeCents
            }
        }
    "#;

    // No rakeCents provided — should default to 0
    let variables = Variables::from_json(json!({
        "input": {
            "clubId": club_id.to_string(),
            "name": "No Rake Tournament",
            "startTime": "2026-06-01T18:00:00Z",
            "buyInCents": 5000
        }
    }));

    let response = execute_graphql(&schema, mutation, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Create tournament without rake should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["createTournament"]["rakeCents"], 0);
}

#[tokio::test]
async fn test_update_tournament_rake() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "rake_update_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Rake Update Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    // Create tournament with default rake (0)
    let tournament_id = create_test_tournament(&app_state, club_id, "Rake Update Tournament").await;

    // Update rake to 500
    let mutation = r#"
        mutation UpdateTournament($input: UpdateTournamentInput!) {
            updateTournament(input: $input) {
                id
                rakeCents
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "id": tournament_id.to_string(),
            "rakeCents": 500
        }
    }));

    let response = execute_graphql(&schema, mutation, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Update tournament rake should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["updateTournament"]["rakeCents"], 500);
}
