//! G2 invariant: the two economies are sealed.
//!
//! Euros (cosmetics) and Prediction Points must never share a transaction path.
//! This test inspects the live schema and asserts there is NO foreign key, in
//! either direction, between the euro tables and the PP tables. If anyone ever
//! adds such a column, this test fails — the firewall is enforced by CI, not
//! just by convention.

use crate::common::setup_test_db;

/// The euro economy (cosmetics) tables.
const EURO_TABLES: &[&str] = &["cosmetic_item", "user_cosmetic", "cosmetic_purchase"];
/// The Prediction-Points economy tables.
const PP_TABLES: &[&str] = &["prediction_point_ledger", "prediction_entry"];

#[tokio::test]
async fn euro_and_prediction_points_share_no_foreign_key() {
    let state = setup_test_db().await;

    // Every foreign key in the public schema: (from_table -> to_table).
    let fks: Vec<(String, String)> = sqlx::query_as(
        "SELECT c1.relname AS from_table, c2.relname AS to_table \
         FROM pg_constraint con \
         JOIN pg_class c1 ON c1.oid = con.conrelid \
         JOIN pg_class c2 ON c2.oid = con.confrelid \
         WHERE con.contype = 'f'",
    )
    .fetch_all(&state.db)
    .await
    .expect("failed to read foreign keys");

    let is_euro = |t: &str| EURO_TABLES.contains(&t);
    let is_pp = |t: &str| PP_TABLES.contains(&t);

    let crossing: Vec<&(String, String)> = fks
        .iter()
        .filter(|(from, to)| (is_euro(from) && is_pp(to)) || (is_pp(from) && is_euro(to)))
        .collect();

    assert!(
        crossing.is_empty(),
        "G2 firewall breached — foreign key(s) connect the euro and Prediction-Points economies: {crossing:?}"
    );
}
