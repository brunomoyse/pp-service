mod common;

use api::gql::build_schema;
use async_graphql::Variables;
use common::*;
use serde_json::json;

#[tokio::test]
async fn test_get_users_query() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (_, claims) = create_test_user(&app_state, "adminuser@test.com", "admin").await;

    let query = r#"
        query GetUsers($limit: Int, $offset: Int) {
            users(limit: $limit, offset: $offset) {
                id
                email
                firstName
                lastName
                role
                isActive
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "limit": 10,
        "offset": 0
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(claims)).await;

    assert!(
        response.errors.is_empty(),
        "Users query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let users = data["users"].as_array().unwrap();

    assert!(!users.is_empty(), "Should return at least one user");
}

#[tokio::test]
async fn test_get_user_by_id() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (user_id, claims) = create_test_user(&app_state, "specificuser@test.com", "admin").await;

    let query = r#"
        query GetUsers($limit: Int, $offset: Int) {
            users(limit: $limit, offset: $offset) {
                id
                email
                role
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "limit": 100,
        "offset": 0
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(claims)).await;

    assert!(
        response.errors.is_empty(),
        "User query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let users = data["users"].as_array().unwrap();

    // Find our specific user
    let user = users
        .iter()
        .find(|u| u["id"] == user_id.to_string())
        .expect("User should be found");

    assert_eq!(user["id"], user_id.to_string());
    assert_eq!(user["email"], "specificuser@test.com");
}
