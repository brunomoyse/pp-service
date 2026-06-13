//! Prize-pool integrity for `enterTournamentResults`: payouts are driven by the
//! DB-maintained pool (entries) and must reconcile to it exactly, or the whole
//! mutation rolls back (Tier 1 single-source-of-truth + reconciliation guard).

use crate::common::*;
use api::gql::build_schema;
use async_graphql::Variables;
use serde_json::json;
use uuid::Uuid;

type TestSchema =
    async_graphql::Schema<api::gql::QueryRoot, api::gql::MutationRoot, api::gql::SubscriptionRoot>;

const BUY_IN: i32 = 5000;

async fn add_initial_entry(
    schema: &TestSchema,
    tournament_id: Uuid,
    player_id: Uuid,
    claims: &api::auth::Claims,
) {
    let mutation = r#"
        mutation AddEntry($input: AddTournamentEntryInput!) {
            addTournamentEntry(input: $input) { id }
        }
    "#;
    let vars = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "userId": player_id.to_string(),
            "entryType": "INITIAL",
            "amountCents": BUY_IN,
            "chipsReceived": 10000
        }
    }));
    let r = execute_graphql(schema, mutation, Some(vars), Some(claims.clone())).await;
    assert!(r.errors.is_empty(), "entry should be added: {:?}", r.errors);
}

/// Manager + club + tournament + three funded players (pool = 3 × BUY_IN).
async fn three_funded_players(
    prefix: &str,
) -> (
    api::AppState,
    TestSchema,
    Uuid,
    api::auth::Claims,
    Vec<Uuid>,
) {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());

    let (manager_id, manager_claims) =
        create_test_user(&app_state, &format!("{prefix}_mgr@test.com"), "manager").await;
    let club_id = create_test_club(&app_state, &format!("{prefix} Club")).await;
    create_club_manager(&app_state, manager_id, club_id).await;
    let tournament_id =
        create_test_tournament(&app_state, club_id, &format!("{prefix} Tournament")).await;

    let mut players = Vec::new();
    for i in 1..=3 {
        let (pid, _) =
            create_test_user(&app_state, &format!("{prefix}_p{i}@test.com"), "player").await;
        add_initial_entry(&schema, tournament_id, pid, &manager_claims).await;
        players.push(pid);
    }

    (app_state, schema, tournament_id, manager_claims, players)
}

async fn prize_pool(app_state: &api::AppState, tournament_id: Uuid) -> i32 {
    sqlx::query_scalar("SELECT total_prize_pool FROM tournament_payouts WHERE tournament_id = $1")
        .bind(tournament_id)
        .fetch_one(&app_state.db)
        .await
        .expect("payout row exists once entries are recorded")
}

#[tokio::test]
async fn results_distribute_exactly_the_prize_pool() {
    let (app_state, schema, tournament_id, manager_claims, players) =
        three_funded_players("money_ok").await;

    // The pool is the DB-maintained sum of entries, not buy_in × players.
    let pool = prize_pool(&app_state, tournament_id).await;
    assert_eq!(pool, 3 * BUY_IN, "pool reflects the three recorded entries");

    let mutation = r#"
        mutation EnterResults($input: EnterTournamentResultsInput!) {
            enterTournamentResults(input: $input) {
                success
                results { finalPosition prizeCents }
            }
        }
    "#;
    // Even split across all three so the distribution is deterministic.
    let vars = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "playerPositions": [
                { "userId": players[0].to_string(), "finalPosition": 1 },
                { "userId": players[1].to_string(), "finalPosition": 2 },
                { "userId": players[2].to_string(), "finalPosition": 3 }
            ],
            "deal": { "dealType": "EVEN_SPLIT", "affectedPositions": [1, 2, 3] }
        }
    }));

    let resp = execute_graphql(&schema, mutation, Some(vars), Some(manager_claims)).await;
    assert!(
        resp.errors.is_empty(),
        "entering results should succeed: {:?}",
        resp.errors
    );

    let data = resp.data.into_json().unwrap();
    let results = data["enterTournamentResults"]["results"]
        .as_array()
        .unwrap();
    let distributed: i64 = results
        .iter()
        .map(|r| r["prizeCents"].as_i64().unwrap())
        .sum();

    // The core invariant: every cent of the pool is paid out, no more, no less.
    assert_eq!(
        distributed, pool as i64,
        "distributed payouts must reconcile to the prize pool"
    );
}

#[tokio::test]
async fn funded_results_without_a_payout_structure_are_rejected_and_rolled_back() {
    let (app_state, schema, tournament_id, manager_claims, players) =
        three_funded_players("money_guard").await;
    let pool = prize_pool(&app_state, tournament_id).await;
    assert_eq!(pool, 3 * BUY_IN);

    let mutation = r#"
        mutation EnterResults($input: EnterTournamentResultsInput!) {
            enterTournamentResults(input: $input) { success }
        }
    "#;
    // No payout template and no deal: nothing can be distributed, so the
    // reconciliation guard must reject a non-zero pool rather than record
    // finishes that pay out nothing.
    let vars = Variables::from_json(json!({
        "input": {
            "tournamentId": tournament_id.to_string(),
            "playerPositions": [
                { "userId": players[0].to_string(), "finalPosition": 1 },
                { "userId": players[1].to_string(), "finalPosition": 2 },
                { "userId": players[2].to_string(), "finalPosition": 3 }
            ]
        }
    }));

    let resp = execute_graphql(&schema, mutation, Some(vars), Some(manager_claims)).await;
    assert!(
        !resp.errors.is_empty(),
        "a funded tournament cannot record results that distribute nothing"
    );

    // The guard fires before persistence — no partial results leak through.
    let persisted: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tournament_results WHERE tournament_id = $1")
            .bind(tournament_id)
            .fetch_one(&app_state.db)
            .await
            .unwrap();
    assert_eq!(
        persisted, 0,
        "no results persisted on reconciliation failure"
    );
}
