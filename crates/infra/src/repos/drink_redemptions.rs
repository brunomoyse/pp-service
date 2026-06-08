use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::DrinkRedemptionRow;

const COLUMNS: &str =
    "id, wallet_id, bar_station_id, drink_type, idempotency_key, created_by, created_at";

/// Find an existing redemption for a wallet + idempotency key. A retried scan with
/// the same key resolves to the original row instead of debiting again.
pub async fn find_by_idempotency<'e>(
    executor: impl PgExecutor<'e>,
    wallet_id: Uuid,
    idempotency_key: &str,
) -> SqlxResult<Option<DrinkRedemptionRow>> {
    sqlx::query_as::<_, DrinkRedemptionRow>(&format!(
        "SELECT {COLUMNS} FROM drink_redemption \
         WHERE wallet_id = $1 AND idempotency_key = $2"
    ))
    .bind(wallet_id)
    .bind(idempotency_key)
    .fetch_optional(executor)
    .await
}

/// Insert a redemption row. A concurrent retry with the same `(wallet_id,
/// idempotency_key)` raises a unique violation, which the caller treats as a dedupe.
pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    wallet_id: Uuid,
    bar_station_id: Uuid,
    drink_type: Option<&str>,
    idempotency_key: &str,
    created_by: Option<Uuid>,
) -> SqlxResult<DrinkRedemptionRow> {
    sqlx::query_as::<_, DrinkRedemptionRow>(&format!(
        "INSERT INTO drink_redemption \
         (wallet_id, bar_station_id, drink_type, idempotency_key, created_by) \
         VALUES ($1, $2, $3, $4, $5) RETURNING {COLUMNS}"
    ))
    .bind(wallet_id)
    .bind(bar_station_id)
    .bind(drink_type)
    .bind(idempotency_key)
    .bind(created_by)
    .fetch_one(executor)
    .await
}
