//! Personal performance analytics for the Pro tier.
//!
//! Each query is scoped to a single user. Buy-ins come from registrations
//! (one per tournament entered); winnings come from results (LEFT JOIN, so a
//! tournament with no cash still counts its buy-in). Net = winnings − buy-ins.

use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ClubBreakdownRow {
    pub club_id: Uuid,
    pub club_name: String,
    pub tournaments: i64,
    pub buyins_cents: i64,
    pub winnings_cents: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BuyInBreakdownRow {
    pub buy_in_cents: i32,
    pub tournaments: i64,
    pub buyins_cents: i64,
    pub winnings_cents: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PnlPointRow {
    pub day: chrono::NaiveDate,
    pub net_cents: i64,
}

// `clubs` joins 1:1 on every tournament, so including it here is harmless for
// the by-buy-in and timeline queries that don't reference it.
const BASE_FROM: &str = "FROM tournament_registrations reg \
     JOIN tournaments t ON reg.tournament_id = t.id \
     JOIN clubs c ON c.id = t.club_id \
     LEFT JOIN tournament_results tr \
        ON tr.tournament_id = reg.tournament_id AND tr.user_id = reg.user_id \
     WHERE reg.user_id = $1";

pub async fn by_club<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
) -> SqlxResult<Vec<ClubBreakdownRow>> {
    sqlx::query_as::<_, ClubBreakdownRow>(&format!(
        "SELECT t.club_id AS club_id, c.name AS club_name, \
                COUNT(*) AS tournaments, \
                COALESCE(SUM(t.buy_in_cents), 0) AS buyins_cents, \
                COALESCE(SUM(tr.prize_cents), 0) AS winnings_cents \
         {BASE_FROM} \
         GROUP BY t.club_id, c.name \
         ORDER BY tournaments DESC",
    ))
    .bind(user_id)
    .fetch_all(executor)
    .await
}

pub async fn by_buy_in<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
) -> SqlxResult<Vec<BuyInBreakdownRow>> {
    sqlx::query_as::<_, BuyInBreakdownRow>(&format!(
        "SELECT t.buy_in_cents AS buy_in_cents, \
                COUNT(*) AS tournaments, \
                COALESCE(SUM(t.buy_in_cents), 0) AS buyins_cents, \
                COALESCE(SUM(tr.prize_cents), 0) AS winnings_cents \
         {BASE_FROM} \
         GROUP BY t.buy_in_cents \
         ORDER BY t.buy_in_cents ASC",
    ))
    .bind(user_id)
    .fetch_all(executor)
    .await
}

pub async fn pnl_timeline<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
) -> SqlxResult<Vec<PnlPointRow>> {
    sqlx::query_as::<_, PnlPointRow>(&format!(
        "SELECT t.start_time::date AS day, \
                COALESCE(SUM(tr.prize_cents), 0) - COALESCE(SUM(t.buy_in_cents), 0) AS net_cents \
         {BASE_FROM} \
         GROUP BY day \
         ORDER BY day ASC",
    ))
    .bind(user_id)
    .fetch_all(executor)
    .await
}
