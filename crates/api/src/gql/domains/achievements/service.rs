use infra::repos::achievements;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnlockedAchievement {
    pub code: String,
    pub name_key: String,
}

/// Evaluate all achievements for a player after a tournament
/// Returns list of newly unlocked achievements
pub async fn evaluate_for_player<'a>(
    tx: &mut sqlx::Transaction<'a, sqlx::Postgres>,
    user_id: Uuid,
    tournament_id: Uuid,
) -> Result<Vec<UnlockedAchievement>, Box<dyn std::error::Error + Send + Sync>> {
    let mut newly_unlocked = Vec::new();

    // We'll compute stats inline without using the impl PgExecutor pattern
    // Compute aggregate stats for the user
    let stats_row = sqlx::query(
        r#"
        SELECT
            COUNT(DISTINCT tr.tournament_id) as total_tournaments_with_results,
            SUM(CASE WHEN tr.prize_cents > 0 THEN 1 ELSE 0 END)::i32 as itm_count,
            SUM(CASE WHEN tr.final_position = 1 THEN 1 ELSE 0 END)::i32 as wins,
            SUM(CASE WHEN tr.final_position <= 9 THEN 1 ELSE 0 END)::i32 as final_tables,
            COALESCE(SUM(tr.prize_cents), 0)::i32 as total_winnings_cents
        FROM tournament_results tr
        WHERE tr.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await?;

    let total_tournaments_with_results: i64 = stats_row.get("total_tournaments_with_results");
    let itm_count: i32 = stats_row.get("itm_count");
    let wins: i32 = stats_row.get("wins");
    let final_tables: i32 = stats_row.get("final_tables");
    let total_winnings_cents: i32 = stats_row.get("total_winnings_cents");

    // Count total tournament participations (registrations, not busted/no-show/cancelled)
    let participation_row = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM tournament_registrations
        WHERE user_id = $1 AND status NOT IN ('cancelled', 'no_show')
        "#,
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await?;
    let total_participation = participation_row as i32;

    // Calculate ITM rate
    let itm_rate = if total_tournaments_with_results > 0 {
        (itm_count as f64 * 100.0 / total_tournaments_with_results as f64) as i32
    } else {
        0
    };

    // Count rebuys in this tournament
    let rebuy_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM tournament_entries
        WHERE user_id = $1 AND tournament_id = $2 AND entry_type = 'rebuy'
        "#,
    )
    .bind(user_id)
    .bind(tournament_id)
    .fetch_one(&mut **tx)
    .await?;

    // Compute streaks: order tournaments by start_time DESC, compute consecutive cash/play runs
    let streak_rows = sqlx::query(
        r#"
        SELECT tr.prize_cents
        FROM tournament_results tr
        JOIN tournaments t ON tr.tournament_id = t.id
        WHERE tr.user_id = $1
        ORDER BY t.start_time DESC
        LIMIT 10
        "#,
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut current_cash_streak = 0;
    for row in streak_rows {
        let prize: i32 = row.get("prize_cents");
        if prize > 0 {
            current_cash_streak += 1;
        } else {
            break;
        }
    }

    // Play streak: consecutive recent tournament participations (no busted/no_show/cancelled)
    let play_streak_rows = sqlx::query(
        r#"
        SELECT tr.user_id
        FROM tournament_registrations tr
        JOIN tournaments t ON tr.tournament_id = t.id
        WHERE tr.user_id = $1 AND tr.status NOT IN ('cancelled', 'no_show')
        ORDER BY t.start_time DESC
        LIMIT 10
        "#,
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;

    let current_play_streak = play_streak_rows.len() as i32;

    // Evaluate each achievement
    let catalog = achievements::list_catalog(&mut **tx).await?;

    for achievement in catalog {
        let achievement_id = achievement.id;
        let code = achievement.code.clone();
        let threshold = achievement.threshold_value;

        let (progress, should_unlock) = match code.as_str() {
            "first_registration" => {
                // Boolean: has any registration
                let unlocked = total_participation > 0;
                (0, unlocked)
            }
            "first_cash" => {
                // Boolean: has won anything
                let unlocked = total_winnings_cents > 0;
                (0, unlocked)
            }
            "first_win" => {
                // Boolean: has a 1st place
                let unlocked = wins > 0;
                (0, unlocked)
            }
            "tournaments_5" => {
                // Progress: total participation
                (total_participation, threshold.map(|t| total_participation >= t).unwrap_or(false))
            }
            "tournaments_20" => {
                (total_participation, threshold.map(|t| total_participation >= t).unwrap_or(false))
            }
            "tournaments_50" => {
                (total_participation, threshold.map(|t| total_participation >= t).unwrap_or(false))
            }
            "final_table_5" => {
                (final_tables, threshold.map(|t| final_tables >= t).unwrap_or(false))
            }
            "winnings_1000" => {
                // threshold_value is in cents
                (total_winnings_cents, threshold.map(|t| total_winnings_cents >= t).unwrap_or(false))
            }
            "itm_rate_50" => {
                // threshold_value is percentage
                (itm_rate, threshold.map(|t| itm_rate >= t).unwrap_or(false))
            }
            "rebuy_3" => {
                (rebuy_count as i32, threshold.map(|t| rebuy_count as i32 >= t).unwrap_or(false))
            }
            "streak_cash_3" => {
                (current_cash_streak, threshold.map(|t| current_cash_streak >= t).unwrap_or(false))
            }
            "streak_play_5" => {
                (current_play_streak, threshold.map(|t| current_play_streak >= t).unwrap_or(false))
            }
            _ => (0, false),
        };

        // Update progress
        achievements::upsert_progress(&mut **tx, user_id, achievement_id, progress).await?;

        // Unlock if condition met
        if should_unlock {
            let newly_unlocked_bool =
                achievements::unlock(&mut **tx, user_id, achievement_id, Some(tournament_id))
                    .await?;

            if newly_unlocked_bool {
                newly_unlocked.push(UnlockedAchievement {
                    code,
                    name_key: achievement.name_key,
                });
            }
        }
    }

    Ok(newly_unlocked)
}
