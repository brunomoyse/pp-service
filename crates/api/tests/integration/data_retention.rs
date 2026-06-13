//! Data-retention sweep: the query that selects dormant accounts and the
//! anonymization it then applies (the same path self-service deletion uses).

use infra::repos::users;

use crate::common::{create_test_user, setup_test_db};

/// Force a user's activity timestamp to a fixed age in days.
async fn set_last_seen_days_ago(db: &sqlx::PgPool, user_id: uuid::Uuid, days: i32) {
    sqlx::query("UPDATE users SET last_seen_at = NOW() - make_interval(days => $2) WHERE id = $1")
        .bind(user_id)
        .bind(days)
        .execute(db)
        .await
        .expect("failed to set last_seen_at");
}

#[tokio::test]
async fn retention_selects_only_dormant_players() {
    let app = setup_test_db().await;
    let db = &app.db;

    // Dormant player (4 years quiet) — should be selected.
    let (dormant, _) = create_test_user(&app, "retention-dormant@test.dev", "player").await;
    set_last_seen_days_ago(db, dormant, 1500).await;

    // Recently-active player — must NOT be selected.
    let (active, _) = create_test_user(&app, "retention-active@test.dev", "player").await;
    set_last_seen_days_ago(db, active, 10).await;

    // Long-dormant manager — staff is out of scope, must NOT be selected.
    let (manager, _) = create_test_user(&app, "retention-manager@test.dev", "manager").await;
    set_last_seen_days_ago(db, manager, 2000).await;

    let ids = users::find_inactive_player_ids(db, 1095, 100)
        .await
        .expect("query failed");

    assert!(
        ids.contains(&dormant),
        "dormant player should be a candidate"
    );
    assert!(!ids.contains(&active), "active player must be excluded");
    assert!(!ids.contains(&manager), "manager (staff) must be excluded");
}

#[tokio::test]
async fn retention_anonymizes_pii_but_keeps_row() {
    let app = setup_test_db().await;
    let db = &app.db;

    let (user_id, _) = create_test_user(&app, "retention-anon@test.dev", "player").await;

    let before = users::get_by_id(db, user_id)
        .await
        .expect("query failed")
        .expect("user exists");
    assert!(before.is_active);

    users::anonymize(db, user_id)
        .await
        .expect("anonymize failed")
        .expect("user existed");

    let after = users::get_by_id(db, user_id)
        .await
        .expect("query failed")
        .expect("row is retained, not deleted");
    assert!(!after.is_active, "anonymized account is deactivated");
    assert_ne!(after.email, before.email, "email is scrubbed");
    assert!(
        after.email.ends_with("@anonymized.invalid"),
        "email points to the anonymized sentinel domain"
    );
    assert!(after.username.is_none(), "username is cleared");
}

#[tokio::test]
async fn touch_last_seen_clears_dormancy() {
    let app = setup_test_db().await;
    let db = &app.db;

    let (user_id, _) = create_test_user(&app, "retention-touch@test.dev", "player").await;
    set_last_seen_days_ago(db, user_id, 1500).await;

    // A login/refresh heartbeat lands; the account is no longer a candidate.
    users::touch_last_seen(db, user_id)
        .await
        .expect("touch failed");

    let ids = users::find_inactive_player_ids(db, 1095, 100)
        .await
        .expect("query failed");
    assert!(
        !ids.contains(&user_id),
        "a freshly-active account must drop out of the dormant set"
    );
}
