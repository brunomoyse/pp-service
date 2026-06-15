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

const CREATE_RECURRING_MUTATION: &str = r#"
    mutation CreateTournament($input: CreateTournamentInput!) {
        createTournament(input: $input) {
            id
            startTime
        }
    }
"#;

#[tokio::test]
async fn test_create_weekly_recurring_tournaments() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "weekly_recur_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Weekly Recurrence Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    // Start + 4 weeks out (inclusive) => 5 occurrences, 7 days apart. A custom
    // 2-level structure is provided so we can assert it is copied to each.
    let variables = Variables::from_json(json!({
        "input": {
            "clubId": club_id.to_string(),
            "name": "Weekly Deepstack",
            "startTime": "2026-01-01T19:00:00Z",
            "buyInCents": 5000,
            "recurrenceFrequency": "WEEKLY",
            "recurrenceEndDate": "2026-01-29T19:00:00Z",
            "structure": [
                { "levelNumber": 1, "smallBlind": 100, "bigBlind": 200, "ante": 0, "durationMinutes": 20, "isBreak": false },
                { "levelNumber": 2, "smallBlind": 200, "bigBlind": 400, "ante": 0, "durationMinutes": 20, "isBreak": false }
            ]
        }
    }));

    let response = execute_graphql(
        &schema,
        CREATE_RECURRING_MUTATION,
        Some(variables),
        Some(manager_claims),
    )
    .await;
    assert!(
        response.errors.is_empty(),
        "Create weekly recurring tournament should succeed: {:?}",
        response.errors
    );

    // All occurrences for the club, oldest first.
    let starts: Vec<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT start_time FROM tournaments WHERE club_id = $1 ORDER BY start_time ASC",
    )
    .bind(club_id)
    .fetch_all(&app_state.db)
    .await
    .expect("query tournament start times");

    assert_eq!(starts.len(), 5, "expected 5 weekly occurrences");
    for w in starts.windows(2) {
        assert_eq!(
            w[1] - w[0],
            chrono::Duration::days(7),
            "occurrences should be 7 days apart"
        );
    }

    // The provided 2-level structure is copied to every occurrence.
    for &start in &starts {
        let levels: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tournament_structures ts
             JOIN tournaments t ON t.id = ts.tournament_id
             WHERE t.club_id = $1 AND t.start_time = $2",
        )
        .bind(club_id)
        .bind(start)
        .fetch_one(&app_state.db)
        .await
        .expect("count structure levels");
        assert_eq!(
            levels, 2,
            "each occurrence should copy the 2-level structure"
        );
    }
}

#[tokio::test]
async fn test_create_monthly_recurring_tournaments() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "monthly_recur_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Monthly Recurrence Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    // Jan 15 .. Apr 15 monthly => 4 occurrences, day-of-month preserved.
    let variables = Variables::from_json(json!({
        "input": {
            "clubId": club_id.to_string(),
            "name": "Monthly Main Event",
            "startTime": "2026-01-15T20:00:00Z",
            "buyInCents": 10000,
            "recurrenceFrequency": "MONTHLY",
            "recurrenceEndDate": "2026-04-15T20:00:00Z"
        }
    }));

    let response = execute_graphql(
        &schema,
        CREATE_RECURRING_MUTATION,
        Some(variables),
        Some(manager_claims),
    )
    .await;
    assert!(
        response.errors.is_empty(),
        "Create monthly recurring tournament should succeed: {:?}",
        response.errors
    );

    let starts: Vec<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT start_time FROM tournaments WHERE club_id = $1 ORDER BY start_time ASC",
    )
    .bind(club_id)
    .fetch_all(&app_state.db)
    .await
    .expect("query tournament start times");

    let expected: Vec<chrono::DateTime<chrono::Utc>> = [
        "2026-01-15T20:00:00Z",
        "2026-02-15T20:00:00Z",
        "2026-03-15T20:00:00Z",
        "2026-04-15T20:00:00Z",
    ]
    .iter()
    .map(|s| s.parse().unwrap())
    .collect();
    assert_eq!(
        starts, expected,
        "monthly occurrences should preserve day-of-month"
    );
}

#[tokio::test]
async fn test_create_tournament_without_recurrence_makes_one() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, "no_recur_manager@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "No Recurrence Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let variables = Variables::from_json(json!({
        "input": {
            "clubId": club_id.to_string(),
            "name": "One Off",
            "startTime": "2026-01-01T19:00:00Z",
            "buyInCents": 5000
        }
    }));

    let response = execute_graphql(
        &schema,
        CREATE_RECURRING_MUTATION,
        Some(variables),
        Some(manager_claims),
    )
    .await;
    assert!(
        response.errors.is_empty(),
        "Create non-recurring tournament should succeed: {:?}",
        response.errors
    );

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tournaments WHERE club_id = $1")
        .bind(club_id)
        .fetch_one(&app_state.db)
        .await
        .expect("count tournaments");
    assert_eq!(
        count, 1,
        "no recurrence should create exactly one tournament"
    );
}
