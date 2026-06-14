use crate::common::*;
use api::gql::build_schema;
use api::AppState;
use async_graphql::Variables;
use serde_json::json;
use uuid::Uuid;

const CREATE_TABLE: &str = r#"
    mutation CreateTable($input: CreateClubTableInput!) {
        createClubTable(input: $input) {
            id
            tableNumber
            maxSeats
            isDefault
            isActive
        }
    }
"#;

const CREATE_TOURNAMENT: &str = r#"
    mutation CreateTournament($input: CreateTournamentInput!) {
        createTournament(input: $input) { id }
    }
"#;

/// Tables assigned (active) to a tournament, by number, ascending.
async fn linked_table_numbers(app_state: &AppState, tournament_id: Uuid) -> Vec<i32> {
    sqlx::query_scalar::<_, i32>(
        r#"
        SELECT ct.table_number
        FROM tournament_table_assignments tta
        JOIN club_tables ct ON ct.id = tta.club_table_id
        WHERE tta.tournament_id = $1 AND tta.is_active = true
        ORDER BY ct.table_number ASC
        "#,
    )
    .bind(tournament_id)
    .fetch_all(&app_state.db)
    .await
    .expect("failed to read linked tables")
}

async fn make_manager_and_club(app_state: &AppState, tag: &str) -> (api::auth::Claims, Uuid) {
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let (manager_id, claims) = create_test_user(
        app_state,
        &format!("{tag}_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(app_state, &format!("{tag} Club {unique}")).await;
    create_club_manager(app_state, manager_id, club_id).await;
    (claims, club_id)
}

/// A manager can predefine a table; a duplicate number is rejected.
#[tokio::test]
async fn test_create_club_table_and_reject_duplicate() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());
    let (claims, club_id) = make_manager_and_club(&app_state, "tbl_create").await;

    let vars = Variables::from_json(json!({
        "input": { "clubId": club_id.to_string(), "tableNumber": 1, "maxSeats": 8 }
    }));
    let response = execute_graphql(&schema, CREATE_TABLE, Some(vars), Some(claims.clone())).await;
    assert!(
        response.errors.is_empty(),
        "create table: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    assert_eq!(data["createClubTable"]["tableNumber"], 1);
    assert_eq!(data["createClubTable"]["maxSeats"], 8);
    assert_eq!(data["createClubTable"]["isDefault"], true);

    // Same number again → rejected.
    let dup = Variables::from_json(json!({
        "input": { "clubId": club_id.to_string(), "tableNumber": 1 }
    }));
    let response = execute_graphql(&schema, CREATE_TABLE, Some(dup), Some(claims)).await;
    assert!(
        !response.errors.is_empty(),
        "duplicate table number should fail"
    );
    assert!(
        response.errors[0]
            .message
            .to_lowercase()
            .contains("already exists"),
        "expected 'already exists', got: '{}'",
        response.errors[0].message
    );
}

/// The default set is auto-linked when a tournament is created; non-default
/// tables are left out.
#[tokio::test]
async fn test_default_tables_auto_link_on_create() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());
    let (claims, club_id) = make_manager_and_club(&app_state, "tbl_autolink").await;

    for (number, is_default) in [(1, true), (2, true), (3, false)] {
        let vars = Variables::from_json(json!({
            "input": {
                "clubId": club_id.to_string(),
                "tableNumber": number,
                "isDefault": is_default
            }
        }));
        let r = execute_graphql(&schema, CREATE_TABLE, Some(vars), Some(claims.clone())).await;
        assert!(r.errors.is_empty(), "seed table {number}: {:?}", r.errors);
    }

    let vars = Variables::from_json(json!({
        "input": {
            "clubId": club_id.to_string(),
            "name": "Auto-link Cup",
            "startTime": chrono::Utc::now().to_rfc3339(),
            "buyInCents": 5000
        }
    }));
    let response = execute_graphql(&schema, CREATE_TOURNAMENT, Some(vars), Some(claims)).await;
    assert!(
        response.errors.is_empty(),
        "create tournament: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    let tournament_id = Uuid::parse_str(data["createTournament"]["id"].as_str().unwrap()).unwrap();

    assert_eq!(
        linked_table_numbers(&app_state, tournament_id).await,
        vec![1, 2],
        "only default tables should be auto-linked"
    );
}

/// A table booked by a live tournament cannot be assigned to another, and the
/// second tournament's auto-link skips the conflicting table.
#[tokio::test]
async fn test_active_table_conflict_is_blocked() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());
    let (claims, club_id) = make_manager_and_club(&app_state, "tbl_conflict").await;

    // One default table.
    let vars = Variables::from_json(json!({
        "input": { "clubId": club_id.to_string(), "tableNumber": 7 }
    }));
    let created = execute_graphql(&schema, CREATE_TABLE, Some(vars), Some(claims.clone()))
        .await
        .data
        .into_json()
        .unwrap();
    let table_id = created["createClubTable"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Tournament A auto-links table 7.
    let vars = Variables::from_json(json!({
        "input": {
            "clubId": club_id.to_string(),
            "name": "Tournament A",
            "startTime": chrono::Utc::now().to_rfc3339(),
            "buyInCents": 5000
        }
    }));
    let a = execute_graphql(&schema, CREATE_TOURNAMENT, Some(vars), Some(claims.clone()))
        .await
        .data
        .into_json()
        .unwrap();
    let a_id = Uuid::parse_str(a["createTournament"]["id"].as_str().unwrap()).unwrap();
    assert_eq!(linked_table_numbers(&app_state, a_id).await, vec![7]);

    // Tournament B is created while A is live → auto-link skips the busy table.
    let vars = Variables::from_json(json!({
        "input": {
            "clubId": club_id.to_string(),
            "name": "Tournament B",
            "startTime": chrono::Utc::now().to_rfc3339(),
            "buyInCents": 5000
        }
    }));
    let b = execute_graphql(&schema, CREATE_TOURNAMENT, Some(vars), Some(claims.clone()))
        .await
        .data
        .into_json()
        .unwrap();
    let b_id = Uuid::parse_str(b["createTournament"]["id"].as_str().unwrap()).unwrap();
    assert!(
        linked_table_numbers(&app_state, b_id).await.is_empty(),
        "B should not auto-link a table already booked by A"
    );

    // Manually assigning the busy table to B is rejected.
    let assign = r#"
        mutation Assign($input: AssignTableToTournamentInput!) {
            assignTableToTournament(input: $input) { id }
        }
    "#;
    let vars = Variables::from_json(json!({
        "input": { "tournamentId": b_id.to_string(), "clubTableId": table_id }
    }));
    let response = execute_graphql(&schema, assign, Some(vars), Some(claims)).await;
    assert!(
        !response.errors.is_empty(),
        "assigning a busy table should fail"
    );
    assert!(
        response.errors[0]
            .message
            .to_lowercase()
            .contains("already in use"),
        "expected 'already in use', got: '{}'",
        response.errors[0].message
    );
}
