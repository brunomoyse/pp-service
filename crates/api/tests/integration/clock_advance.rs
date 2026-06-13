//! The clock service's *automatic* behavior — the selection queries it ticks on
//! and the stale-finish sweep. (Manual advance/revert via GraphQL are covered in
//! tournament_clock.rs / clock_lifecycle.rs; this pins the time-driven paths the
//! 5s loop can't be unit-tested against directly.)

use crate::common::*;
use infra::repos::{tournament_clock, tournaments};
use uuid::Uuid;

async fn seed_structures(db: &sqlx::PgPool, tid: Uuid) {
    sqlx::query(
        "INSERT INTO tournament_structures
            (tournament_id, level_number, small_blind, big_blind, ante, duration_minutes)
         VALUES ($1,1,25,50,0,20),($1,2,50,100,0,20),($1,3,100,200,0,20)
         ON CONFLICT DO NOTHING",
    )
    .bind(tid)
    .execute(db)
    .await
    .expect("seed structures");
}

/// Force a running clock to a known level with `level_end_time` `secs` from now
/// (negative = already expired). Ensures a clock row exists first (the tournament
/// trigger may already have created one), then sets the exact state.
async fn set_clock(db: &sqlx::PgPool, tid: Uuid, level: i32, secs: f64) {
    let _ = tournament_clock::create_clock(db, tid).await;
    sqlx::query(
        "UPDATE tournament_clocks
         SET clock_status='running', current_level=$2, auto_advance=true,
             level_started_at=NOW(), pause_started_at=NULL,
             level_end_time = NOW() + make_interval(secs => $3)
         WHERE tournament_id=$1",
    )
    .bind(tid)
    .bind(level)
    .bind(secs)
    .execute(db)
    .await
    .expect("set clock state");
}

#[tokio::test]
async fn expired_running_level_advances_to_a_fresh_future_level() {
    let app = setup_test_db().await;
    let db = &app.db;
    let club_id = create_test_club(&app, "Clock Club").await;
    let tid = create_test_tournament(&app, club_id, "Clock Tournament").await;
    seed_structures(db, tid).await;
    set_clock(db, tid, 1, -60.0).await; // level 1, ended a minute ago

    // The service's selection query picks up the expired running level.
    let due = tournament_clock::get_tournaments_to_advance(db)
        .await
        .unwrap();
    assert!(
        due.contains(&tid),
        "an expired running level should be due for advance"
    );

    // Advancing moves to level 2 with a fresh, FUTURE end time...
    let clock = tournament_clock::advance_level(db, tid, true, None)
        .await
        .unwrap();
    assert_eq!(clock.current_level, 2);
    assert!(
        clock.level_end_time.unwrap() > chrono::Utc::now(),
        "advance must set a future level_end_time so the auto-advance loop doesn't spin"
    );

    // ...so it is no longer due on the next tick.
    let due_after = tournament_clock::get_tournaments_to_advance(db)
        .await
        .unwrap();
    assert!(
        !due_after.contains(&tid),
        "a freshly-advanced level must not be immediately due again"
    );
}

#[tokio::test]
async fn expired_final_level_is_flagged_to_stop_not_advance() {
    let app = setup_test_db().await;
    let db = &app.db;
    let club_id = create_test_club(&app, "Final Club").await;
    let tid = create_test_tournament(&app, club_id, "Final Tournament").await;
    seed_structures(db, tid).await; // levels 1..3
    set_clock(db, tid, 3, -60.0).await; // last level (no level 4), expired

    assert!(
        !tournament_clock::get_tournaments_to_advance(db)
            .await
            .unwrap()
            .contains(&tid),
        "with no next level, an expired clock is not an advance candidate"
    );
    assert!(
        tournament_clock::get_tournaments_at_final_level(db)
            .await
            .unwrap()
            .contains(&tid),
        "an expired final level is flagged for the stop path"
    );
}

#[tokio::test]
async fn in_progress_tournament_is_stale_swept_then_finished() {
    let app = setup_test_db().await;
    let db = &app.db;
    let club_id = create_test_club(&app, "Stale Club").await;
    let tid = create_test_tournament(&app, club_id, "Stale Tournament").await;

    sqlx::query("UPDATE tournaments SET live_status='in_progress' WHERE id=$1")
        .bind(tid)
        .execute(db)
        .await
        .unwrap();

    // The 0h window matches any in-progress tournament (updated_at < NOW());
    // the 24h window does not match a just-updated one. Exercises both the
    // status filter and the age predicate without backdating the trigger column.
    let stale_now = tournaments::list_stale(db, 0).await.unwrap();
    assert!(
        stale_now.iter().any(|t| t.id == tid),
        "an in-progress tournament is stale at the 0h boundary"
    );
    let stale_24h = tournaments::list_stale(db, 24).await.unwrap();
    assert!(
        !stale_24h.iter().any(|t| t.id == tid),
        "a freshly-updated tournament is not stale at 24h"
    );

    tournaments::auto_finish(db, tid)
        .await
        .unwrap()
        .expect("a non-finished tournament is finished");
    let status: String =
        sqlx::query_scalar("SELECT live_status::text FROM tournaments WHERE id=$1")
            .bind(tid)
            .fetch_one(db)
            .await
            .unwrap();
    assert_eq!(status, "finished");

    // A finished tournament drops out of the stale set entirely.
    let after = tournaments::list_stale(db, 0).await.unwrap();
    assert!(
        !after.iter().any(|t| t.id == tid),
        "finished tournaments are not stale-swept"
    );
}
