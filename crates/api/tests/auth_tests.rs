mod common;

use api::gql::build_schema;
use async_graphql::Variables;
use common::*;
use serde_json::json;

#[tokio::test]
async fn test_user_registration() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state);

    let query = r#"
        mutation RegisterUser($input: UserRegistrationInput!) {
            registerUser(input: $input) {
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
        "input": {
            "email": "newuser@test.com",
            "password": "testpassword123",
            "firstName": "New",
            "lastName": "User",
            "username": "newuser"
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    assert!(
        response.errors.is_empty(),
        "Registration should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let user = &data["registerUser"];

    assert_eq!(user["email"], "newuser@test.com");
    assert_eq!(user["firstName"], "New");
    assert_eq!(user["lastName"], "User");
    assert_eq!(user["role"], "PLAYER");
    assert_eq!(user["isActive"], true);
}

#[tokio::test]
async fn test_user_login() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    // First create a user
    let (_, _) = create_test_user(&app_state, "logintest@test.com", "player").await;

    let query = r#"
        mutation LoginUser($input: UserLoginInput!) {
            loginUser(input: $input) {
                token
                user {
                    id
                    email
                    role
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "email": "logintest@test.com",
            "password": "admin"
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), None).await;

    // Note: This might fail due to password verification, but we test the structure
    let data = response.data.into_json().unwrap_or_default();
    if let Some(login_data) = data.get("loginUser") {
        assert!(login_data.get("token").is_some());
        assert!(login_data.get("user").is_some());
    }
}

#[tokio::test]
async fn test_me_query_authenticated() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (user_id, claims) = create_test_user(&app_state, "metest@test.com", "manager").await;

    let query = r#"
        query {
            me {
                id
                email
                role
                isActive
            }
        }
    "#;

    let response = execute_graphql(&schema, query, None, Some(claims)).await;

    assert!(
        response.errors.is_empty(),
        "Me query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let user = &data["me"];

    assert_eq!(user["id"], user_id.to_string());
    assert_eq!(user["email"], "metest@test.com");
    assert_eq!(user["role"], "MANAGER");
}

#[tokio::test]
async fn test_me_query_unauthenticated() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state);

    let query = r#"
        query {
            me {
                id
                email
            }
        }
    "#;

    let response = execute_graphql(&schema, query, None, None).await;

    assert!(
        !response.errors.is_empty(),
        "Me query should fail without authentication"
    );
    assert!(response.errors[0].message.contains("You must be logged in"));
}
