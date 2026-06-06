use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::RivalryRow;

/// Head-to-head records for a player against everyone they've shared a final
/// table sheet with. A "meeting" is a tournament both finished; a lower
/// `final_position` is better, so the subject "wins" a meeting by finishing
/// above the opponent. Ordered most-played first.
pub async fn for_user<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
    limit: i64,
) -> SqlxResult<Vec<RivalryRow>> {
    sqlx::query_as::<_, RivalryRow>(
        "SELECT r2.user_id AS opponent_id, \
                COALESCE(u.username, u.first_name) AS opponent_name, \
                COUNT(*) AS meetings, \
                SUM(CASE WHEN r1.final_position < r2.final_position THEN 1 ELSE 0 END) AS wins, \
                SUM(CASE WHEN r1.final_position > r2.final_position THEN 1 ELSE 0 END) AS losses \
         FROM tournament_results r1 \
         JOIN tournament_results r2 \
              ON r2.tournament_id = r1.tournament_id AND r2.user_id <> r1.user_id \
         JOIN users u ON u.id = r2.user_id \
         WHERE r1.user_id = $1 \
         GROUP BY r2.user_id, u.username, u.first_name \
         ORDER BY meetings DESC, losses DESC \
         LIMIT $2",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(executor)
    .await
}
