use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::{AchievementRow, PlayerAchievementRow};

/// List all achievements in the catalog, ordered by tier and name
pub async fn list_catalog<'e>(
    executor: impl PgExecutor<'e>,
) -> SqlxResult<Vec<AchievementRow>> {
    sqlx::query_as::<_, AchievementRow>(
        r#"
        SELECT id, code, name_key, description_key, category, icon, tier, threshold_value, metadata, created_at, updated_at
        FROM achievements
        ORDER BY tier ASC, name_key ASC
        "#,
    )
    .fetch_all(executor)
    .await
}

/// Get an achievement by its code
pub async fn get_by_code<'e>(
    executor: impl PgExecutor<'e>,
    code: &str,
) -> SqlxResult<Option<AchievementRow>> {
    sqlx::query_as::<_, AchievementRow>(
        r#"
        SELECT id, code, name_key, description_key, category, icon, tier, threshold_value, metadata, created_at, updated_at
        FROM achievements
        WHERE code = $1
        "#,
    )
    .bind(code)
    .fetch_optional(executor)
    .await
}

/// Get an achievement by its ID
pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<AchievementRow>> {
    sqlx::query_as::<_, AchievementRow>(
        r#"
        SELECT id, code, name_key, description_key, category, icon, tier, threshold_value, metadata, created_at, updated_at
        FROM achievements
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Flat row for LEFT JOIN query
#[derive(sqlx::FromRow)]
struct PlayerAchievementJoinRow {
    pub a_id: Uuid,
    pub a_code: String,
    pub a_name_key: String,
    pub a_description_key: String,
    pub a_category: String,
    pub a_icon: Option<String>,
    pub a_tier: Option<String>,
    pub a_threshold_value: Option<i32>,
    pub a_metadata: Option<serde_json::Value>,
    pub a_created_at: chrono::DateTime<chrono::Utc>,
    pub a_updated_at: chrono::DateTime<chrono::Utc>,

    pub pa_id: Option<Uuid>,
    pub pa_unlocked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub pa_progress: Option<i32>,
    pub pa_tournament_id: Option<Uuid>,
    pub pa_metadata: Option<serde_json::Value>,
    pub pa_created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub pa_updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// List all achievements for a player with their progress (LEFT JOIN includes locked achievements)
pub async fn list_for_player<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
) -> SqlxResult<Vec<(AchievementRow, Option<PlayerAchievementRow>)>> {
    let rows = sqlx::query_as::<_, PlayerAchievementJoinRow>(
        r#"
        SELECT
            a.id AS a_id,
            a.code AS a_code,
            a.name_key AS a_name_key,
            a.description_key AS a_description_key,
            a.category AS a_category,
            a.icon AS a_icon,
            a.tier AS a_tier,
            a.threshold_value AS a_threshold_value,
            a.metadata AS a_metadata,
            a.created_at AS a_created_at,
            a.updated_at AS a_updated_at,
            pa.id AS pa_id,
            pa.unlocked_at AS pa_unlocked_at,
            pa.progress AS pa_progress,
            pa.tournament_id AS pa_tournament_id,
            pa.metadata AS pa_metadata,
            pa.created_at AS pa_created_at,
            pa.updated_at AS pa_updated_at
        FROM achievements a
        LEFT JOIN player_achievements pa ON a.id = pa.achievement_id AND pa.user_id = $1
        ORDER BY a.tier ASC, a.name_key ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let achievement = AchievementRow {
                id: row.a_id,
                code: row.a_code,
                name_key: row.a_name_key,
                description_key: row.a_description_key,
                category: row.a_category,
                icon: row.a_icon,
                tier: row.a_tier,
                threshold_value: row.a_threshold_value,
                metadata: row.a_metadata,
                created_at: row.a_created_at,
                updated_at: row.a_updated_at,
            };

            let player_achievement = row.pa_id.map(|id| PlayerAchievementRow {
                id,
                user_id,
                achievement_id: row.a_id,
                unlocked_at: row.pa_unlocked_at,
                progress: row.pa_progress.unwrap_or(0),
                tournament_id: row.pa_tournament_id,
                metadata: row.pa_metadata,
                created_at: row.pa_created_at.unwrap_or(chrono::Utc::now()),
                updated_at: row.pa_updated_at.unwrap_or(chrono::Utc::now()),
            });

            (achievement, player_achievement)
        })
        .collect())
}

/// Upsert progress for a player's achievement (keeps max progress, does not clear unlocked_at)
pub async fn upsert_progress<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
    achievement_id: Uuid,
    progress: i32,
) -> SqlxResult<()> {
    sqlx::query(
        r#"
        INSERT INTO player_achievements (user_id, achievement_id, progress)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, achievement_id) DO UPDATE SET
            progress = GREATEST(player_achievements.progress, EXCLUDED.progress),
            updated_at = NOW()
        "#,
    )
    .bind(user_id)
    .bind(achievement_id)
    .bind(progress)
    .execute(executor)
    .await?;

    Ok(())
}

/// Unlock an achievement for a player (sets unlocked_at if not already set)
/// Returns true if newly unlocked, false if already unlocked
pub async fn unlock<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
    achievement_id: Uuid,
    tournament_id: Option<Uuid>,
) -> SqlxResult<bool> {
    // Use a query that returns whether this action newly set unlocked_at
    // We return 1 if unlocked_at was NULL and we just set it, 0 if it was already set
    let was_newly_unlocked = sqlx::query_scalar::<_, i32>(
        r#"
        INSERT INTO player_achievements (user_id, achievement_id, progress, tournament_id, unlocked_at)
        VALUES ($1, $2, 1, $3, NOW())
        ON CONFLICT (user_id, achievement_id) DO UPDATE SET
            unlocked_at = COALESCE(player_achievements.unlocked_at, NOW()),
            progress = GREATEST(player_achievements.progress, EXCLUDED.progress),
            updated_at = NOW()
        RETURNING CASE WHEN player_achievements.unlocked_at IS NULL THEN 1 ELSE 0 END
        "#,
    )
    .bind(user_id)
    .bind(achievement_id)
    .bind(tournament_id)
    .fetch_one(executor)
    .await?;

    Ok(was_newly_unlocked == 1)
}
