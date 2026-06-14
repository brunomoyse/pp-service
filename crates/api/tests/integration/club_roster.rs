use crate::common::*;
use api::gql::build_schema;
use async_graphql::Variables;
use serde_json::json;

const CREATE: &str = r#"
    mutation Create($input: CreateClubPlayerInput!) {
        createClubPlayer(input: $input) {
            id
            displayName
            firstName
            lastName
            isClaimed
            isActive
        }
    }
"#;

/// Creating a roster entry from a structured first/last name composes the
/// display name as "First Last" and stores the parts.
#[tokio::test]
async fn test_create_roster_entry_splits_name() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("roster_split_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Roster Split Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let variables = Variables::from_json(json!({
        "input": { "clubId": club_id.to_string(), "firstName": "Jean", "lastName": "Dupont" }
    }));
    let response = execute_graphql(&schema, CREATE, Some(variables), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Create roster entry should succeed: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    let player = &data["createClubPlayer"];
    assert_eq!(player["firstName"], "Jean");
    assert_eq!(player["lastName"], "Dupont");
    assert_eq!(player["displayName"], "Jean Dupont");
    assert_eq!(player["isClaimed"], false);
    assert_eq!(player["isActive"], true);
}

/// Renaming a roster entry updates both the structured parts and the composed
/// display name.
#[tokio::test]
async fn test_update_roster_entry_renames() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("roster_rename_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Roster Rename Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let create_vars = Variables::from_json(json!({
        "input": { "clubId": club_id.to_string(), "firstName": "Bob", "lastName": "Martin" }
    }));
    let created = execute_graphql(
        &schema,
        CREATE,
        Some(create_vars),
        Some(manager_claims.clone()),
    )
    .await
    .data
    .into_json()
    .unwrap();
    let player_id = created["createClubPlayer"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let update = r#"
        mutation Update($input: UpdateClubPlayerInput!) {
            updateClubPlayer(input: $input) { id displayName firstName lastName }
        }
    "#;
    let update_vars = Variables::from_json(json!({
        "input": { "id": player_id, "firstName": "Robert", "lastName": "Martin" }
    }));
    let response = execute_graphql(&schema, update, Some(update_vars), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Rename should succeed: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    assert_eq!(data["updateClubPlayer"]["firstName"], "Robert");
    assert_eq!(data["updateClubPlayer"]["displayName"], "Robert Martin");
}

/// Anonymising an unclaimed roster entry scrubs the name and deactivates it.
#[tokio::test]
async fn test_anonymize_unclaimed_entry() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("roster_anon_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Roster Anon Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let create_vars = Variables::from_json(json!({
        "input": { "clubId": club_id.to_string(), "firstName": "Temp", "lastName": "Guest" }
    }));
    let created = execute_graphql(
        &schema,
        CREATE,
        Some(create_vars),
        Some(manager_claims.clone()),
    )
    .await
    .data
    .into_json()
    .unwrap();
    let player_id = created["createClubPlayer"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let anonymize = r#"
        mutation Anon($id: ID!) {
            anonymizeClubPlayer(id: $id) { id displayName isActive }
        }
    "#;
    let vars = Variables::from_json(json!({ "id": player_id }));
    let response = execute_graphql(&schema, anonymize, Some(vars), Some(manager_claims)).await;

    assert!(
        response.errors.is_empty(),
        "Anonymise should succeed: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    assert_eq!(
        data["anonymizeClubPlayer"]["displayName"],
        "Anonymous Player"
    );
    assert_eq!(data["anonymizeClubPlayer"]["isActive"], false);
}

/// A roster entry linked to an app account cannot be anonymised through the
/// manager path — it belongs to the user.
#[tokio::test]
async fn test_anonymize_claimed_entry_refused() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("roster_claimed_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Roster Claimed Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    let create_vars = Variables::from_json(json!({
        "input": { "clubId": club_id.to_string(), "firstName": "Real", "lastName": "User" }
    }));
    let created = execute_graphql(
        &schema,
        CREATE,
        Some(create_vars),
        Some(manager_claims.clone()),
    )
    .await
    .data
    .into_json()
    .unwrap();
    let player_id = created["createClubPlayer"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Link the entry to an app user (simulate a claim).
    let (app_user_id, _) = create_test_user(
        &app_state,
        &format!("roster_claimer_{unique}@test.com"),
        "player",
    )
    .await;
    sqlx::query("UPDATE club_player SET app_user_id = $1 WHERE id = $2")
        .bind(app_user_id)
        .bind(uuid::Uuid::parse_str(&player_id).unwrap())
        .execute(&app_state.db)
        .await
        .expect("Failed to link roster entry to app user");

    let anonymize = r#"
        mutation Anon($id: ID!) {
            anonymizeClubPlayer(id: $id) { id }
        }
    "#;
    let vars = Variables::from_json(json!({ "id": player_id }));
    let response = execute_graphql(&schema, anonymize, Some(vars), Some(manager_claims)).await;

    assert!(
        !response.errors.is_empty(),
        "Anonymising a claimed entry should fail"
    );
    assert!(
        response.errors[0]
            .message
            .to_lowercase()
            .contains("app account"),
        "Expected an 'app account' refusal, got: '{}'",
        response.errors[0].message
    );
}

/// The roster lists entries ordered by family name ascending.
#[tokio::test]
async fn test_roster_ordered_by_family_name() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, manager_claims) = create_test_user(
        &app_state,
        &format!("roster_order_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(&app_state, "Roster Order Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;

    for (first, last) in [("Alice", "Zulu"), ("Bob", "Alpha"), ("Carol", "Mike")] {
        let vars = Variables::from_json(json!({
            "input": { "clubId": club_id.to_string(), "firstName": first, "lastName": last }
        }));
        let r = execute_graphql(&schema, CREATE, Some(vars), Some(manager_claims.clone())).await;
        assert!(r.errors.is_empty(), "seed create failed: {:?}", r.errors);
    }

    let list = r#"
        query List($clubId: ID!) {
            clubPlayers(clubId: $clubId) { lastName }
        }
    "#;
    let vars = Variables::from_json(json!({ "clubId": club_id.to_string() }));
    let response = execute_graphql(&schema, list, Some(vars), Some(manager_claims)).await;
    assert!(
        response.errors.is_empty(),
        "list failed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let names: Vec<String> = data["clubPlayers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["lastName"].as_str().unwrap_or("").to_string())
        .collect();
    assert_eq!(
        names,
        vec!["Alpha", "Mike", "Zulu"],
        "roster not ordered by family name"
    );
}
