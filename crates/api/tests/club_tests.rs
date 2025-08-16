mod common;

use api::gql::build_schema;
use async_graphql::Variables;
use common::*;
use serde_json::json;

#[tokio::test]
async fn test_get_clubs_query() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let club_id = create_test_club(&app_state, "Test Poker Club").await;

    let query = r#"
        query {
            clubs {
                id
                name
                city
            }
        }
    "#;

    let response = execute_graphql(&schema, query, None, None).await;

    assert!(
        response.errors.is_empty(),
        "Clubs query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let clubs = data["clubs"].as_array().unwrap();

    assert!(!clubs.is_empty(), "Should return at least one club");

    // Find our test club
    let test_club = clubs.iter().find(|c| c["id"] == club_id.to_string());
    assert!(test_club.is_some(), "Should find our test club");
}

#[tokio::test]
async fn test_get_club_by_id() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let club_id = create_test_club(&app_state, "Specific Test Club").await;

    let query = r#"
        query GetClub($clubId: ID!) {
            club(id: $clubId) {
                id
                name
                city
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "clubId": club_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Club query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let club = &data["club"];

    assert_eq!(club["id"], club_id.to_string());
    assert_eq!(club["name"], "Specific Test Club");
}
