//! Authorization guards added in Tier 1: the live-state reads now require a
//! logged-in caller, and the financial `tournamentEntries` query is manager-only.
//! These are one-line `ctx.data::<Claims>()` / `require_club_manager` checks that
//! are easy to drop in a refactor — so we assert the *rejection*, not just success.

use crate::common::*;
use api::gql::build_schema;
use async_graphql::Variables;
use serde_json::json;

type TestSchema =
    async_graphql::Schema<api::gql::QueryRoot, api::gql::MutationRoot, api::gql::SubscriptionRoot>;

fn is_unauthenticated(resp: &async_graphql::Response) -> bool {
    resp.errors.iter().any(|e| {
        // `auth_error()` sets this exact message and extensions.code.
        e.message.contains("Authentication required")
            || e.extensions
                .as_ref()
                .and_then(|x| x.get("code"))
                .map(|v| matches!(v, async_graphql::Value::String(s) if s == "UNAUTHENTICATED"))
                .unwrap_or(false)
    })
}

/// A login-required read called with no Claims must be rejected as UNAUTHENTICATED.
/// These resolvers gate on Claims before any DB work, so a random id is fine.
async fn assert_login_required(
    schema: &TestSchema,
    label: &str,
    query: &str,
    vars: serde_json::Value,
) {
    let resp = execute_graphql(schema, query, Some(Variables::from_json(vars)), None).await;
    assert!(
        is_unauthenticated(&resp),
        "{label}: an unauthenticated call must be rejected as UNAUTHENTICATED; got errors={:?}",
        resp.errors
    );
}

#[tokio::test]
async fn unauthenticated_live_reads_are_rejected() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());
    let tid = uuid::Uuid::new_v4().to_string();
    let table_id = uuid::Uuid::new_v4().to_string();

    assert_login_required(
        &schema,
        "tournamentEntryStats",
        r#"query($id: ID!){ tournamentEntryStats(tournamentId: $id){ totalEntries } }"#,
        json!({ "id": tid }),
    )
    .await;

    assert_login_required(
        &schema,
        "tournamentSeatingChart",
        r#"query($id: ID!){ tournamentSeatingChart(tournamentId: $id){ tournament { id } } }"#,
        json!({ "id": tid }),
    )
    .await;

    assert_login_required(
        &schema,
        "tournamentTables",
        r#"query($id: ID!){ tournamentTables(tournamentId: $id){ id } }"#,
        json!({ "id": tid }),
    )
    .await;

    assert_login_required(
        &schema,
        "tableSeatAssignments",
        r#"query($id: ID!){ tableSeatAssignments(clubTableId: $id){ assignment { id } } }"#,
        json!({ "id": table_id }),
    )
    .await;

    assert_login_required(
        &schema,
        "tournamentSeatingHistory",
        r#"query($id: ID!){ tournamentSeatingHistory(tournamentId: $id, limit: 10){ id } }"#,
        json!({ "id": tid }),
    )
    .await;

    assert_login_required(
        &schema,
        "tournamentBounties",
        r#"query($id: ID!){ tournamentBounties(tournamentId: $id){ id } }"#,
        json!({ "id": tid }),
    )
    .await;
}

#[tokio::test]
async fn tournament_entries_is_manager_only() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, _) = create_test_user(&app_state, "authz_mgr@test.com", "manager").await;
    let club_id = create_test_club(&app_state, "Authz Club").await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id = create_test_tournament(&app_state, club_id, "Authz Tournament").await;

    let query = r#"query($id: ID!){ tournamentEntries(tournamentId: $id){ id } }"#;
    let vars = || Variables::from_json(json!({ "id": tournament_id.to_string() }));

    // Unauthenticated: rejected (no financial data leaks).
    let anon = execute_graphql(&schema, query, Some(vars()), None).await;
    assert!(
        !anon.errors.is_empty(),
        "an unauthenticated tournamentEntries query must be rejected"
    );

    // A logged-in NON-manager player: denied — entries are financial data.
    let (_, player_claims) = create_test_user(&app_state, "authz_player@test.com", "player").await;
    let denied = execute_graphql(&schema, query, Some(vars()), Some(player_claims)).await;
    assert!(
        !denied.errors.is_empty(),
        "a non-manager player must be denied tournamentEntries"
    );
}
