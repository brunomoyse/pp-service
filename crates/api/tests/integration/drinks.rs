use api::gql::build_schema;
use api::gql::domains::drinks::service::{
    self, ActivateParams, ClaimParams, RedeemParams, TopUpParams,
};
use api::AppState;
use async_graphql::Variables;
use chrono::Duration;
use infra::repos::{bar_stations, drink_wallet_credentials, drink_wallets};
use serde_json::json;
use uuid::Uuid;

use crate::common::{
    create_club_manager, create_test_club, create_test_user, execute_graphql, setup_test_db,
};

/// Create a club with an active manager and return (club_id, manager_id, manager_claims).
async fn setup_club_with_manager(app_state: &AppState) -> (Uuid, Uuid, api::auth::Claims) {
    let unique = Uuid::new_v4();
    let (manager_id, claims) = create_test_user(
        app_state,
        &format!("drink_mgr_{unique}@test.com"),
        "manager",
    )
    .await;
    let club_id = create_test_club(app_state, &format!("Drink Club {unique}")).await;
    create_club_manager(app_state, manager_id, club_id).await;
    (club_id, manager_id, claims)
}

/// Generate one printed card and activate it into a (bearer) wallet seeded with
/// `initial` credits. Returns (raw_token, credential_id, wallet).
async fn activate_wallet(
    app_state: &AppState,
    club_id: Uuid,
    operator: Uuid,
    initial: Option<i32>,
) -> (String, Uuid, infra::models::DrinkWalletRow) {
    let cards = service::generate_printed_cards(&app_state.db, 1)
        .await
        .expect("generate cards");
    let card = cards.into_iter().next().expect("one card");

    let outcome = service::activate_printed_card(
        &app_state.db,
        ActivateParams {
            raw_token: card.token.clone(),
            club_id,
            display_name: None,
            initial_top_up: initial,
            expires_at: None,
            operator_user_id: operator,
        },
    )
    .await
    .expect("activate card");

    (card.token, card.credential_id, outcome.wallet)
}

async fn ledger_sum(app_state: &AppState, wallet_id: Uuid) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(delta), 0) FROM drink_ledger_entry WHERE wallet_id = $1",
    )
    .bind(wallet_id)
    .fetch_one(&app_state.db)
    .await
    .expect("sum ledger")
}

async fn count_rows(app_state: &AppState, sql: &str, wallet_id: Uuid) -> i64 {
    sqlx::query_scalar::<_, i64>(sql)
        .bind(wallet_id)
        .fetch_one(&app_state.db)
        .await
        .expect("count rows")
}

/// Concurrent redemptions on one wallet must never overspend: with a balance of 5 and
/// 10 simultaneous scans (distinct idempotency keys), exactly 5 succeed and the
/// balance bottoms out at 0, never negative.
#[tokio::test]
async fn test_concurrent_redeem_never_overspends() {
    let app_state = setup_test_db().await;
    let (club_id, manager_id, _) = setup_club_with_manager(&app_state).await;
    let station = bar_stations::create(&app_state.db, club_id, "Main Bar")
        .await
        .expect("create station");
    let (token, _cred, wallet) = activate_wallet(&app_state, club_id, manager_id, Some(5)).await;

    let mut handles = Vec::new();
    for i in 0..10 {
        let pool = app_state.db.clone();
        let token = token.clone();
        let station_id = station.id;
        handles.push(tokio::spawn(async move {
            service::redeem_drink(
                &pool,
                RedeemParams {
                    raw_token: token,
                    bar_station_id: station_id,
                    idempotency_key: format!("scan-{i}"),
                    drink_type: None,
                    operator_user_id: manager_id,
                },
            )
            .await
        }));
    }

    let mut succeeded = 0;
    for handle in handles {
        if matches!(handle.await, Ok(Ok(outcome)) if !outcome.deduped) {
            succeeded += 1;
        }
    }

    assert_eq!(succeeded, 5, "exactly 5 of 10 scans should debit");

    let wallet = drink_wallets::get_by_id(&app_state.db, wallet.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(wallet.balance, 0, "balance must bottom out at 0");
    assert_eq!(ledger_sum(&app_state, wallet.id).await, 0);
    assert_eq!(
        count_rows(
            &app_state,
            "SELECT COUNT(*) FROM drink_redemption WHERE wallet_id = $1",
            wallet.id
        )
        .await,
        5,
        "exactly 5 redemption rows"
    );
}

/// A retried scan with the same idempotency key debits exactly once.
#[tokio::test]
async fn test_redeem_idempotent_on_retry() {
    let app_state = setup_test_db().await;
    let (club_id, manager_id, _) = setup_club_with_manager(&app_state).await;
    let station = bar_stations::create(&app_state.db, club_id, "Bar")
        .await
        .unwrap();
    let (token, _cred, wallet) = activate_wallet(&app_state, club_id, manager_id, Some(3)).await;

    let params = |key: &str| RedeemParams {
        raw_token: token.clone(),
        bar_station_id: station.id,
        idempotency_key: key.to_string(),
        drink_type: None,
        operator_user_id: manager_id,
    };

    let first = service::redeem_drink(&app_state.db, params("same-key"))
        .await
        .expect("first redeem");
    assert!(!first.deduped);
    assert_eq!(first.balance, 2);

    let second = service::redeem_drink(&app_state.db, params("same-key"))
        .await
        .expect("retry redeem");
    assert!(second.deduped, "retry must be deduped");
    assert_eq!(second.balance, 2, "retry must not debit again");

    let wallet = drink_wallets::get_by_id(&app_state.db, wallet.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(wallet.balance, 2);
    assert_eq!(
        count_rows(
            &app_state,
            "SELECT COUNT(*) FROM drink_redemption WHERE wallet_id = $1",
            wallet.id
        )
        .await,
        1,
        "exactly one redemption row"
    );
    assert_eq!(ledger_sum(&app_state, wallet.id).await, 2);
}

/// claimCard binds an owner to the card's existing wallet without moving the balance.
#[tokio::test]
async fn test_claim_card_does_not_move_balance() {
    let app_state = setup_test_db().await;
    let (club_id, manager_id, _) = setup_club_with_manager(&app_state).await;
    let unique = Uuid::new_v4();
    let (player_id, _) = create_test_user(
        &app_state,
        &format!("drink_player_{unique}@test.com"),
        "player",
    )
    .await;

    // Bearer wallet with 4 credits, no owner.
    let (token, _cred, wallet) = activate_wallet(&app_state, club_id, manager_id, Some(4)).await;
    assert!(wallet.registered_player_id.is_none());

    let ledger_before = ledger_sum(&app_state, wallet.id).await;
    let entries_before = count_rows(
        &app_state,
        "SELECT COUNT(*) FROM drink_ledger_entry WHERE wallet_id = $1",
        wallet.id,
    )
    .await;

    let outcome = service::claim_card(
        &app_state.db,
        ClaimParams {
            raw_token: token,
            app_user_id: player_id,
            display_name: "Claiming Player".to_string(),
        },
    )
    .await
    .expect("claim card");

    assert_eq!(outcome.wallet.balance, 4, "balance unchanged after claim");
    assert!(
        outcome.wallet.registered_player_id.is_some(),
        "wallet now has an owner"
    );

    // Same wallet, now owned by the player; ledger untouched.
    let wallet = drink_wallets::get_by_id(&app_state.db, wallet.id)
        .await
        .unwrap()
        .unwrap();
    let roster = infra::repos::registered_players::get_by_id(
        &app_state.db,
        wallet.registered_player_id.unwrap(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(roster.app_user_id, Some(player_id));
    assert_eq!(ledger_sum(&app_state, wallet.id).await, ledger_before);
    assert_eq!(
        count_rows(
            &app_state,
            "SELECT COUNT(*) FROM drink_ledger_entry WHERE wallet_id = $1",
            wallet.id
        )
        .await,
        entries_before,
        "no ledger entries added by claim"
    );
}

/// A revoked credential cannot redeem.
#[tokio::test]
async fn test_revoked_credential_cannot_redeem() {
    let app_state = setup_test_db().await;
    let (club_id, manager_id, _) = setup_club_with_manager(&app_state).await;
    let station = bar_stations::create(&app_state.db, club_id, "Bar")
        .await
        .unwrap();
    let (token, credential_id, wallet) =
        activate_wallet(&app_state, club_id, manager_id, Some(2)).await;

    drink_wallet_credentials::revoke(&app_state.db, credential_id)
        .await
        .expect("revoke")
        .expect("credential existed");

    let result = service::redeem_drink(
        &app_state.db,
        RedeemParams {
            raw_token: token,
            bar_station_id: station.id,
            idempotency_key: "after-revoke".to_string(),
            drink_type: None,
            operator_user_id: manager_id,
        },
    )
    .await;

    assert!(result.is_err(), "revoked credential must not redeem");

    let wallet = drink_wallets::get_by_id(&app_state.db, wallet.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(wallet.balance, 2, "balance untouched");
}

/// The expiry job posts correct negative entries for unredeemed expired credits and
/// leaves balance = SUM(delta). It is also re-run safe.
#[tokio::test]
async fn test_expiry_posts_negatives_and_is_idempotent() {
    let app_state = setup_test_db().await;
    let (club_id, manager_id, _) = setup_club_with_manager(&app_state).await;
    let (_token, _cred, wallet) = activate_wallet(&app_state, club_id, manager_id, None).await;

    let past = chrono::Utc::now() - Duration::hours(1);

    // Lot A: 5 credits, already expired.
    service::top_up_wallet(
        &app_state.db,
        TopUpParams {
            wallet_id: wallet.id,
            amount: 5,
            tournament_id: None,
            expires_at: Some(past),
            operator_user_id: manager_id,
        },
    )
    .await
    .expect("top up A");

    // Lot B: 3 credits, never expires.
    service::top_up_wallet(
        &app_state.db,
        TopUpParams {
            wallet_id: wallet.id,
            amount: 3,
            tournament_id: None,
            expires_at: None,
            operator_user_id: manager_id,
        },
    )
    .await
    .expect("top up B");

    let now = chrono::Utc::now();
    let expired = service::run_expiry(&app_state.db, now)
        .await
        .expect("run expiry");
    assert_eq!(expired, 5, "the expired lot's 5 credits should expire");

    let wallet_after = drink_wallets::get_by_id(&app_state.db, wallet.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(wallet_after.balance, 3);
    assert_eq!(
        ledger_sum(&app_state, wallet.id).await,
        3,
        "balance must equal SUM(delta)"
    );
    assert_eq!(
        count_rows(
            &app_state,
            "SELECT COUNT(*) FROM drink_ledger_entry WHERE wallet_id = $1 AND reason = 'expiry' AND delta = -5",
            wallet.id,
        )
        .await,
        1,
        "exactly one -5 expiry entry"
    );

    // Re-running must not double-expire.
    let again = service::run_expiry(&app_state.db, now)
        .await
        .expect("rerun expiry");
    assert_eq!(again, 0, "re-run expires nothing");
    let wallet_final = drink_wallets::get_by_id(&app_state.db, wallet.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(wallet_final.balance, 3);
    assert_eq!(ledger_sum(&app_state, wallet.id).await, 3);
}

/// End-to-end happy path through the GraphQL layer: a manager operating a bar station
/// redeems a drink and the payload reports the new balance.
#[tokio::test]
async fn test_redeem_drink_via_graphql() {
    let app_state = setup_test_db().await;
    let schema = build_schema(app_state.clone());
    let (club_id, manager_id, manager_claims) = setup_club_with_manager(&app_state).await;
    let station = bar_stations::create(&app_state.db, club_id, "Bar")
        .await
        .unwrap();
    let (token, _cred, _wallet) = activate_wallet(&app_state, club_id, manager_id, Some(2)).await;

    let query = r#"
        mutation Redeem($input: RedeemDrinkInput!) {
            redeemDrink(input: $input) {
                walletId
                balance
                deduped
                redemption { id barStationId }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "credentialToken": token,
            "barStationId": station.id.to_string(),
            "idempotencyKey": "gql-scan-1",
            "drinkType": "beer"
        }
    }));

    let response = execute_graphql(&schema, query, Some(variables), Some(manager_claims)).await;
    assert!(
        response.errors.is_empty(),
        "redeem should succeed: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let payload = &data["redeemDrink"];
    assert_eq!(payload["balance"], 1);
    assert_eq!(payload["deduped"], false);
    assert!(!payload["redemption"]["id"].is_null());
}
