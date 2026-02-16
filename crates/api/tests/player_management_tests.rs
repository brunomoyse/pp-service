mod common;

use api::gql::build_schema;
use async_graphql::Variables;
use common::*;
use serde_json::json;

#[tokio::test]
async fn test_create_player() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("createplayer_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Create Player Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let query = r#"
        mutation CreatePlayer($input: CreatePlayerInput!) {
            createPlayer(input: $input) {
                id
                email
                firstName
                lastName
                role
                isActive
            }
        }
    "#;

    let player_email = format!("newplayer_{unique}@test.com");
    let variables = Variables::from_json(json!({
        "input": {
            "email": player_email,
            "firstName": "New",
            "lastName": "Player",
            "clubId": club_id.to_string()
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Create player should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let user = &data["createPlayer"];

    assert_eq!(user["email"], player_email);
    assert_eq!(user["firstName"], "New");
    assert_eq!(user["lastName"], "Player");
    assert_eq!(user["role"], "PLAYER");
    assert_eq!(user["isActive"], true);
}

#[tokio::test]
async fn test_create_player_duplicate_email() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("dupemail_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Dup Email Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    // Create a player with a known email
    let existing_email = format!("existing_{unique}@test.com");
    create_test_user(&app_state, &existing_email, "player").await;

    let query = r#"
        mutation CreatePlayer($input: CreatePlayerInput!) {
            createPlayer(input: $input) {
                id
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "email": existing_email,
            "firstName": "Duplicate",
            "clubId": club_id.to_string()
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Creating player with duplicate email should fail"
    );
    let error_msg = &response.errors[0].message.to_lowercase();
    assert!(
        error_msg.contains("already exists")
            || error_msg.contains("duplicate")
            || error_msg.contains("unique"),
        "Expected duplicate error, got: '{}'",
        response.errors[0].message
    );
}

#[tokio::test]
async fn test_create_player_unauthorized() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (_, player_claims) = create_test_user(
        &app_state,
        &format!("createunauth_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Unauth Create Club").await;

    let query = r#"
        mutation CreatePlayer($input: CreatePlayerInput!) {
            createPlayer(input: $input) {
                id
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "email": format!("unauthplayer_{unique}@test.com"),
            "firstName": "Unauthorized",
            "clubId": club_id.to_string()
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(player_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Player should not be able to create players"
    );
    assert!(
        response.errors[0].message.contains("Access denied"),
        "Expected access denied error, got: '{}'",
        response.errors[0].message
    );
}

#[tokio::test]
async fn test_update_player() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("updateplayer_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let (player_id, _) = create_test_user(
        &app_state,
        &format!("updateplayer_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Update Player Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let query = r#"
        mutation UpdatePlayer($input: UpdatePlayerInput!) {
            updatePlayer(input: $input) {
                id
                firstName
                lastName
                phone
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "id": player_id.to_string(),
            "firstName": "Updated",
            "lastName": "Name",
            "phone": "+1234567890"
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Update player should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let user = &data["updatePlayer"];

    assert_eq!(user["firstName"], "Updated");
    assert_eq!(user["lastName"], "Name");
    assert_eq!(user["phone"], "+1234567890");
}

#[tokio::test]
async fn test_update_player_not_found() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("updatenotfound_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Not Found Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let query = r#"
        mutation UpdatePlayer($input: UpdatePlayerInput!) {
            updatePlayer(input: $input) {
                id
            }
        }
    "#;

    let fake_id = uuid::Uuid::new_v4();
    let variables = Variables::from_json(json!({
        "input": {
            "id": fake_id.to_string(),
            "firstName": "Ghost"
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Updating non-existent player should fail"
    );
    let error_msg = &response.errors[0].message.to_lowercase();
    assert!(
        error_msg.contains("not found") || error_msg.contains("no rows"),
        "Expected 'not found' error, got: '{}'",
        response.errors[0].message
    );
}

#[tokio::test]
async fn test_deactivate_player() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("deactivate_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let (player_id, _) = create_test_user(
        &app_state,
        &format!("deactivate_player_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Deactivate Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let query = r#"
        mutation DeactivatePlayer($id: ID!) {
            deactivatePlayer(id: $id) {
                id
                isActive
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "id": player_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Deactivate player should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["deactivatePlayer"]["isActive"], false);
}

#[tokio::test]
async fn test_reactivate_player() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("reactivate_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let (player_id, _) = create_test_user(
        &app_state,
        &format!("reactivate_player_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Reactivate Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    // Deactivate first
    sqlx::query!(
        "UPDATE users SET is_active = false WHERE id = $1",
        player_id
    )
    .execute(&app_state.db)
    .await
    .expect("Failed to deactivate player");

    let query = r#"
        mutation ReactivatePlayer($id: ID!) {
            reactivatePlayer(id: $id) {
                id
                isActive
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "id": player_id.to_string()
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Reactivate player should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    assert_eq!(data["reactivatePlayer"]["isActive"], true);
}

#[tokio::test]
async fn test_deactivate_reactivate_lifecycle() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("lifecycle_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let (player_id, _) = create_test_user(
        &app_state,
        &format!("lifecycle_player_{unique}@test.com"),
        "player",
    )
    .await;
    let club_id = create_test_club(&app_state, "Lifecycle Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    // 1. Initially active
    let row = sqlx::query!("SELECT is_active FROM users WHERE id = $1", player_id)
        .fetch_one(&app_state.db)
        .await
        .unwrap();
    assert!(row.is_active, "Player should start as active");

    // 2. Deactivate
    let deactivate_query = r#"
        mutation DeactivatePlayer($id: ID!) {
            deactivatePlayer(id: $id) { isActive }
        }
    "#;
    let variables = Variables::from_json(json!({ "id": player_id.to_string() }));
    let response = execute_graphql(
        &schema,
        deactivate_query,
        Some(variables),
        Some(manager_claims.clone()),
    )
    .await;
    assert!(
        response.errors.is_empty(),
        "Deactivate: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    assert_eq!(data["deactivatePlayer"]["isActive"], false);

    // 3. Reactivate
    let reactivate_query = r#"
        mutation ReactivatePlayer($id: ID!) {
            reactivatePlayer(id: $id) { isActive }
        }
    "#;
    let variables = Variables::from_json(json!({ "id": player_id.to_string() }));
    let response = execute_graphql(
        &schema,
        reactivate_query,
        Some(variables),
        Some(manager_claims),
    )
    .await;
    assert!(
        response.errors.is_empty(),
        "Reactivate: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    assert_eq!(data["reactivatePlayer"]["isActive"], true);
}
