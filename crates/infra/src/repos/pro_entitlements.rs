use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::ProEntitlementRow;

const COLS: &str = "id, app_user_id, source, granted_by_club_id, granted_by_user_id, \
                    starts_at, expires_at, status, notes, created_at, updated_at";

/// Whether the user currently holds an active, unexpired entitlement.
pub async fn is_pro<'e>(executor: impl PgExecutor<'e>, app_user_id: Uuid) -> SqlxResult<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (\
            SELECT 1 FROM pro_entitlement \
            WHERE app_user_id = $1 AND status = 'active' \
              AND starts_at <= NOW() \
              AND (expires_at IS NULL OR expires_at > NOW()))",
    )
    .bind(app_user_id)
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// All of a user's entitlements, newest first.
pub async fn list_for_user<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
) -> SqlxResult<Vec<ProEntitlementRow>> {
    sqlx::query_as::<_, ProEntitlementRow>(&format!(
        "SELECT {COLS} FROM pro_entitlement WHERE app_user_id = $1 ORDER BY created_at DESC"
    ))
    .bind(app_user_id)
    .fetch_all(executor)
    .await
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<ProEntitlementRow>> {
    sqlx::query_as::<_, ProEntitlementRow>(&format!(
        "SELECT {COLS} FROM pro_entitlement WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Grant an entitlement (e.g. a club gifting Pro to a regular).
pub async fn grant<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    source: &str,
    granted_by_club_id: Option<Uuid>,
    granted_by_user_id: Option<Uuid>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    notes: Option<&str>,
) -> SqlxResult<ProEntitlementRow> {
    sqlx::query_as::<_, ProEntitlementRow>(&format!(
        "INSERT INTO pro_entitlement \
            (app_user_id, source, granted_by_club_id, granted_by_user_id, expires_at, notes) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING {COLS}"
    ))
    .bind(app_user_id)
    .bind(source)
    .bind(granted_by_club_id)
    .bind(granted_by_user_id)
    .bind(expires_at)
    .bind(notes)
    .fetch_one(executor)
    .await
}

/// Revoke an entitlement. Returns the updated row, or None if it was missing.
pub async fn revoke<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<ProEntitlementRow>> {
    sqlx::query_as::<_, ProEntitlementRow>(&format!(
        "UPDATE pro_entitlement SET status = 'revoked', updated_at = NOW() \
         WHERE id = $1 RETURNING {COLS}"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}
