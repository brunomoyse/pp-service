use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::{FavoriteClubRow, WrappedStatsRow};

/// Tournament totals for a player across one calendar year.
pub async fn stats_for_year<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
    year: i32,
) -> SqlxResult<WrappedStatsRow> {
    sqlx::query_as::<_, WrappedStatsRow>(
        "SELECT COUNT(*) AS tournaments, \
                COALESCE(SUM(t.buy_in_cents), 0) AS buyins_cents, \
                COALESCE(SUM(tr.prize_cents), 0) AS winnings_cents, \
                COALESCE(SUM(CASE WHEN tr.prize_cents > 0 THEN 1 ELSE 0 END), 0) AS itm_count, \
                MIN(tr.final_position) AS best_finish \
         FROM tournament_results tr JOIN tournaments t ON t.id = tr.tournament_id \
         WHERE tr.user_id = $1 AND EXTRACT(YEAR FROM tr.created_at)::int = $2",
    )
    .bind(user_id)
    .bind(year)
    .fetch_one(executor)
    .await
}

/// The club a player entered most that year, if any.
pub async fn favorite_club_for_year<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
    year: i32,
) -> SqlxResult<Option<FavoriteClubRow>> {
    sqlx::query_as::<_, FavoriteClubRow>(
        "SELECT c.name AS club_name, COUNT(*) AS tournaments \
         FROM tournament_results tr \
         JOIN tournaments t ON t.id = tr.tournament_id \
         JOIN clubs c ON c.id = t.club_id \
         WHERE tr.user_id = $1 AND EXTRACT(YEAR FROM tr.created_at)::int = $2 \
         GROUP BY c.name ORDER BY tournaments DESC LIMIT 1",
    )
    .bind(user_id)
    .bind(year)
    .fetch_optional(executor)
    .await
}

/// The opponent who has finished ahead of this player most often across every
/// tournament they both played — the player's lifetime "nemesis", if any. Used
/// by the Year in Poker recap. A lower `final_position` is better, so a "loss"
/// is a tournament where the opponent finished above the player.
pub async fn nemesis_for_user<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
) -> SqlxResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT COALESCE(u.username, u.first_name) AS opponent_name \
         FROM tournament_results r1 \
         JOIN tournament_results r2 \
              ON r2.tournament_id = r1.tournament_id AND r2.user_id <> r1.user_id \
         JOIN users u ON u.id = r2.user_id \
         WHERE r1.user_id = $1 \
         GROUP BY r2.user_id, u.username, u.first_name \
         HAVING SUM(CASE WHEN r1.final_position > r2.final_position THEN 1 ELSE 0 END) > 0 \
         ORDER BY SUM(CASE WHEN r1.final_position > r2.final_position THEN 1 ELSE 0 END) DESC, \
                  COUNT(*) DESC \
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|r| r.0))
}

/// Number of check-ins a player logged that year.
pub async fn check_ins_for_year<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
    year: i32,
) -> SqlxResult<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM check_in \
         WHERE app_user_id = $1 AND EXTRACT(YEAR FROM checked_in_at)::int = $2",
    )
    .bind(user_id)
    .bind(year)
    .fetch_one(executor)
    .await?;
    Ok(row.0)
}
