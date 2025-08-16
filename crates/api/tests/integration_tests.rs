use std::env;

use api::{gql::build_schema, AppState};
use async_graphql::Request;
use sqlx::postgres::PgPoolOptions;

async fn setup_test_db() -> AppState {
    let database_url = env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    AppState::new(pool).expect("Failed to create AppState")
}

#[tokio::test]
async fn test_tournament_live_status_enum() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state);

    // GraphQL query to fetch tournaments with live_status
    let query = r#"
        query {
            tournaments(limit: 5) {
                id
                title
                liveStatus
                status
            }
        }
    "#;

    let request = Request::new(query);
    let response = schema.execute(request).await;

    // Check that the query executed without errors
    if !response.errors.is_empty() {
        panic!("GraphQL errors: {:?}", response.errors);
    }

    // Parse the response
    let data = response.data.into_json().unwrap();
    let tournaments = data["tournaments"].as_array().unwrap();

    // Verify that we have tournaments and they have non-null live_status
    assert!(
        !tournaments.is_empty(),
        "Should have at least one tournament"
    );

    for tournament in tournaments {
        let live_status = &tournament["liveStatus"];

        // Verify live_status is not null and is a valid enum value
        assert!(!live_status.is_null(), "liveStatus should not be null");

        let status_str = live_status.as_str().unwrap();
        assert!(
            matches!(
                status_str,
                "NOT_STARTED"
                    | "REGISTRATION_OPEN"
                    | "LATE_REGISTRATION"
                    | "IN_PROGRESS"
                    | "BREAK"
                    | "FINAL_TABLE"
                    | "FINISHED"
            ),
            "liveStatus should be a valid enum value, got: {}",
            status_str
        );

        println!(
            "Tournament: {} - Status: {}",
            tournament["title"], status_str
        );
    }
}

#[tokio::test]
async fn test_tournament_live_status_specific_values() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state);

    // Query for tournaments with specific filters to get both finished and in_progress
    let query = r#"
        query {
            tournaments(limit: 20) {
                id
                title
                liveStatus
            }
        }
    "#;

    let request = Request::new(query);
    let response = schema.execute(request).await;

    if !response.errors.is_empty() {
        panic!("GraphQL errors: {:?}", response.errors);
    }

    let data = response.data.into_json().unwrap();
    let tournaments = data["tournaments"].as_array().unwrap();

    // Count different status types
    let mut finished_count = 0;
    let mut in_progress_count = 0;

    for tournament in tournaments {
        let status_str = tournament["liveStatus"].as_str().unwrap();
        match status_str {
            "FINISHED" => finished_count += 1,
            "IN_PROGRESS" => in_progress_count += 1,
            _ => {}
        }
    }

    // Based on our seeder, we should have both finished and in_progress tournaments
    assert!(finished_count > 0, "Should have some finished tournaments");
    assert!(
        in_progress_count > 0,
        "Should have some in_progress tournaments"
    );

    println!(
        "Found {} finished and {} in_progress tournaments",
        finished_count, in_progress_count
    );
}

#[tokio::test]
async fn test_tournament_business_status_vs_live_status() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state);

    // Query for tournaments to test both status fields
    let query = r#"
        query {
            tournaments(limit: 10) {
                id
                title
                status
                liveStatus
            }
        }
    "#;

    let request = Request::new(query);
    let response = schema.execute(request).await;

    if !response.errors.is_empty() {
        panic!("GraphQL errors: {:?}", response.errors);
    }

    let data = response.data.into_json().unwrap();
    let tournaments = data["tournaments"].as_array().unwrap();

    assert!(
        !tournaments.is_empty(),
        "Should have at least one tournament"
    );

    for tournament in tournaments {
        let status = tournament["status"].as_str().unwrap();
        let live_status = tournament["liveStatus"].as_str().unwrap();

        // Verify both status fields are valid
        assert!(
            matches!(status, "UPCOMING" | "IN_PROGRESS" | "COMPLETED"),
            "Business status should be a valid enum value, got: {}",
            status
        );

        assert!(
            matches!(
                live_status,
                "NOT_STARTED"
                    | "REGISTRATION_OPEN"
                    | "LATE_REGISTRATION"
                    | "IN_PROGRESS"
                    | "BREAK"
                    | "FINAL_TABLE"
                    | "FINISHED"
            ),
            "Live status should be a valid enum value, got: {}",
            live_status
        );

        // Test the business logic mapping
        match live_status {
            "NOT_STARTED" | "REGISTRATION_OPEN" | "LATE_REGISTRATION" => {
                assert_eq!(
                    status, "UPCOMING",
                    "NOT_STARTED/REGISTRATION should map to UPCOMING business status"
                );
            }
            "IN_PROGRESS" | "BREAK" | "FINAL_TABLE" => {
                assert_eq!(
                    status, "IN_PROGRESS",
                    "IN_PROGRESS/BREAK/FINAL_TABLE should map to IN_PROGRESS business status"
                );
            }
            "FINISHED" => {
                assert_eq!(
                    status, "COMPLETED",
                    "FINISHED should map to COMPLETED business status"
                );
            }
            _ => panic!("Unknown live status: {}", live_status),
        }

        println!(
            "Tournament: {} - Business: {} - Live: {}",
            tournament["title"], status, live_status
        );
    }
}
