use sqlx::{PgExecutor, PgPool, Result as SqlxResult};
use uuid::Uuid;

use crate::models::{CosmeticItemRow, UserCosmeticRow};

const ITEM_COLS: &str =
    "id, code, kind, name, description, price_cents, preview_ref, club_id, active, created_at";
const OWNED_COLS: &str = "id, app_user_id, cosmetic_item_id, source, equipped, acquired_at";

/// Active catalog, optionally filtered to one kind.
pub async fn list_catalog<'e>(
    executor: impl PgExecutor<'e>,
    kind: Option<&str>,
) -> SqlxResult<Vec<CosmeticItemRow>> {
    let sql = format!(
        "SELECT {ITEM_COLS} FROM cosmetic_item \
         WHERE active AND ($1::text IS NULL OR kind = $1) \
         ORDER BY kind, price_cents"
    );
    sqlx::query_as::<_, CosmeticItemRow>(&sql)
        .bind(kind)
        .fetch_all(executor)
        .await
}

pub async fn get_item<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<CosmeticItemRow>> {
    sqlx::query_as::<_, CosmeticItemRow>(&format!(
        "SELECT {ITEM_COLS} FROM cosmetic_item WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// All cosmetics a user owns.
pub async fn list_owned<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
) -> SqlxResult<Vec<UserCosmeticRow>> {
    sqlx::query_as::<_, UserCosmeticRow>(&format!(
        "SELECT {OWNED_COLS} FROM user_cosmetic WHERE app_user_id = $1"
    ))
    .bind(app_user_id)
    .fetch_all(executor)
    .await
}

pub async fn get_owned<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    cosmetic_item_id: Uuid,
) -> SqlxResult<Option<UserCosmeticRow>> {
    sqlx::query_as::<_, UserCosmeticRow>(&format!(
        "SELECT {OWNED_COLS} FROM user_cosmetic \
         WHERE app_user_id = $1 AND cosmetic_item_id = $2"
    ))
    .bind(app_user_id)
    .bind(cosmetic_item_id)
    .fetch_optional(executor)
    .await
}

/// Purchase a cosmetic with euros: records the euro ledger entry AND grants
/// ownership in one transaction. Deterministic — the buyer gets exactly the
/// named item at its listed price (G1). Idempotent on ownership.
pub async fn purchase(
    pool: &PgPool,
    app_user_id: Uuid,
    item: &CosmeticItemRow,
) -> SqlxResult<UserCosmeticRow> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO cosmetic_purchase (app_user_id, cosmetic_item_id, price_cents) \
         VALUES ($1, $2, $3)",
    )
    .bind(app_user_id)
    .bind(item.id)
    .bind(item.price_cents)
    .execute(&mut *tx)
    .await?;

    let owned = sqlx::query_as::<_, UserCosmeticRow>(&format!(
        "INSERT INTO user_cosmetic (app_user_id, cosmetic_item_id, source) \
         VALUES ($1, $2, 'purchase') \
         ON CONFLICT (app_user_id, cosmetic_item_id) DO UPDATE SET source = user_cosmetic.source \
         RETURNING {OWNED_COLS}"
    ))
    .bind(app_user_id)
    .bind(item.id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(owned)
}

/// Grant a cosmetic without euros (club gift / reward). Idempotent.
pub async fn grant<'e>(
    executor: impl PgExecutor<'e>,
    app_user_id: Uuid,
    cosmetic_item_id: Uuid,
    source: &str,
) -> SqlxResult<UserCosmeticRow> {
    sqlx::query_as::<_, UserCosmeticRow>(&format!(
        "INSERT INTO user_cosmetic (app_user_id, cosmetic_item_id, source) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (app_user_id, cosmetic_item_id) DO UPDATE SET source = user_cosmetic.source \
         RETURNING {OWNED_COLS}"
    ))
    .bind(app_user_id)
    .bind(cosmetic_item_id)
    .bind(source)
    .fetch_one(executor)
    .await
}

/// Equip an owned cosmetic, unequipping any other of the same kind.
pub async fn equip(
    pool: &PgPool,
    app_user_id: Uuid,
    cosmetic_item_id: Uuid,
    kind: &str,
) -> SqlxResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE user_cosmetic SET equipped = FALSE \
         WHERE app_user_id = $1 \
           AND cosmetic_item_id IN (SELECT id FROM cosmetic_item WHERE kind = $2)",
    )
    .bind(app_user_id)
    .bind(kind)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE user_cosmetic SET equipped = TRUE \
         WHERE app_user_id = $1 AND cosmetic_item_id = $2",
    )
    .bind(app_user_id)
    .bind(cosmetic_item_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await
}
