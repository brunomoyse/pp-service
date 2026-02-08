mod common;

use api::gql::build_schema;
use async_graphql::Variables;
use common::*;
use serde_json::json;

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
