mod common;

use api::gql::build_schema;
use common::*;

#[tokio::test]
async fn test_server_time_query() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state);

    let query = r#"
        query {
            serverTime
        }
    "#;

    let response = execute_graphql(&schema, query, None, None).await;

    assert!(
        response.errors.is_empty(),
        "Server time query should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert!(
        data["serverTime"].is_string(),
        "Server time should be a string"
    );
}

#[tokio::test]
async fn test_health_check_query() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state);

    // Test a basic query that doesn't require authentication
    let query = r#"
        query {
            clubs {
                id
            }
        }
    "#;

    let response = execute_graphql(&schema, query, None, None).await;

    assert!(
        response.errors.is_empty(),
        "Basic health check query should succeed: {:?}",
        response.errors
    );
}

#[tokio::test]
async fn test_invalid_query() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state);

    let query = r#"
        query {
            nonExistentField
        }
    "#;

    let response = execute_graphql(&schema, query, None, None).await;

    assert!(
        !response.errors.is_empty(),
        "Invalid query should return errors"
    );
    assert!(response.errors[0].message.contains("Cannot query field"));
}

#[tokio::test]
async fn test_malformed_query() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state);

    let query = r#"
        query {
            clubs {
                id
                # missing closing brace
    "#;

    let response = execute_graphql(&schema, query, None, None).await;

    assert!(
        !response.errors.is_empty(),
        "Malformed query should return errors"
    );
}
