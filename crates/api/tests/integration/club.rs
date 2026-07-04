use crate::common::*;
use api::gql::build_schema;

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

    // Find our specific test club
    let test_club = clubs
        .iter()
        .find(|c| c["id"] == club_id.to_string())
        .expect("Should find our test club");

    assert_eq!(test_club["id"], club_id.to_string());
    assert_eq!(test_club["name"], "Specific Test Club");
}

/// Regression: the ClubLoader's hand-written column list drifted from ClubRow
/// (missing address/vat_number/needs_review/plan/subscription columns), which
/// made every loader-resolved `club` field fail at runtime with
/// "no column found for name: address" while the repo queries kept working.
#[tokio::test]
async fn test_tournament_club_resolves_via_loader() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let club_id = create_test_club(&app_state, "Loader Club").await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Loader Tournament").await;

    let query =
        format!(r#"query {{ tournament(id: "{tournament_id}") {{ id club {{ id name }} }} }}"#);

    let response = execute_graphql(&schema, &query, None, None).await;

    assert!(
        response.errors.is_empty(),
        "tournament.club (ClubLoader) should resolve: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["tournament"]["club"]["id"], club_id.to_string());
    assert_eq!(data["tournament"]["club"]["name"], "Loader Club");
}
