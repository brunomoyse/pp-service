//! Refresh-token rotation security: reuse detection (whole-family revocation)
//! and the activity heartbeat that the data-retention job relies on.

use api::auth::refresh::{create_refresh_token, rotate_refresh_token};
use chrono::{DateTime, Utc};

use crate::common::{create_test_user, setup_test_db};

#[tokio::test]
async fn reusing_a_rotated_refresh_token_revokes_the_whole_family() {
    let app = setup_test_db().await;
    let db = &app.db;
    let (user_id, _) = create_test_user(&app, "refresh-theft@test.dev", "player").await;

    let raw1 = create_refresh_token(db, user_id, 7, false)
        .await
        .expect("initial token issued");

    // Legitimate rotation: raw1 is consumed and replaced by raw2.
    let raw2 = rotate_refresh_token(db, &raw1, 7)
        .await
        .expect("first rotation succeeds")
        .new_raw_token;

    // An attacker replays the already-rotated raw1. This must be rejected...
    assert!(
        rotate_refresh_token(db, &raw1, 7).await.is_err(),
        "a reused (already-rotated) token must be rejected"
    );

    // ...and detecting the reuse must revoke the entire family, so the
    // legitimate raw2 the real user holds is now dead too — forcing a re-login.
    assert!(
        rotate_refresh_token(db, &raw2, 7).await.is_err(),
        "reuse detection revokes the whole token family"
    );
}

#[tokio::test]
async fn issuing_a_refresh_token_records_activity() {
    let app = setup_test_db().await;
    let db = &app.db;
    let (user_id, _) = create_test_user(&app, "refresh-lastseen@test.dev", "player").await;

    // A freshly-created user has no recorded activity yet.
    let before: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT last_seen_at FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(db)
            .await
            .unwrap();
    assert!(before.is_none(), "no last_seen_at before any auth event");

    create_refresh_token(db, user_id, 7, false)
        .await
        .expect("token issued");

    // Issuing a token (login) stamps the activity heartbeat the retention job
    // keys off — so an actively-logging-in account is never swept as dormant.
    let after: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT last_seen_at FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(db)
            .await
            .unwrap();
    assert!(after.is_some(), "login records last_seen_at");
}
