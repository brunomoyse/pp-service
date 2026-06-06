use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::UserPrivacySettingsRow;

const COLS: &str = "app_user_id, share_named_pl, in_scouting_pool, created_at, updated_at";

pub async fn get<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
) -> SqlxResult<Option<UserPrivacySettingsRow>> {
    sqlx::query_as::<_, UserPrivacySettingsRow>(&format!(
        "SELECT {COLS} FROM user_privacy_settings WHERE app_user_id = $1"
    ))
    .bind(app_user_id)
    .fetch_optional(executor)
    .await
}

/// Insert or update a user's consent flags.
pub async fn upsert<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    share_named_pl: bool,
    in_scouting_pool: bool,
) -> SqlxResult<UserPrivacySettingsRow> {
    sqlx::query_as::<_, UserPrivacySettingsRow>(&format!(
        "INSERT INTO user_privacy_settings (app_user_id, share_named_pl, in_scouting_pool) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (app_user_id) DO UPDATE SET \
            share_named_pl = EXCLUDED.share_named_pl, \
            in_scouting_pool = EXCLUDED.in_scouting_pool, \
            updated_at = NOW() \
         RETURNING {COLS}"
    ))
    .bind(app_user_id)
    .bind(share_named_pl)
    .bind(in_scouting_pool)
    .fetch_one(executor)
    .await
}

/// Whether a user has opted into the scouting pool (discoverability consent).
pub async fn in_scouting_pool<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
) -> SqlxResult<bool> {
    let row: Option<(bool,)> =
        sqlx::query_as("SELECT in_scouting_pool FROM user_privacy_settings WHERE app_user_id = $1")
            .bind(app_user_id)
            .fetch_optional(executor)
            .await?;
    Ok(row.map(|r| r.0).unwrap_or(false))
}
