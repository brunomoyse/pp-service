use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::DrinkWalletRow;

const COLUMNS: &str = "id, club_player_id, club_id, balance, created_at, updated_at";

/// Get a single wallet by id.
pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<DrinkWalletRow>> {
    sqlx::query_as::<_, DrinkWalletRow>(&format!(
        "SELECT {COLUMNS} FROM drink_wallet WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Get a wallet by id and take a row-level lock (`FOR UPDATE`) so a redemption can
/// read-check-write the cached balance without racing concurrent scans. Must run
/// inside a transaction.
pub async fn get_by_id_for_update<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<DrinkWalletRow>> {
    sqlx::query_as::<_, DrinkWalletRow>(&format!(
        "SELECT {COLUMNS} FROM drink_wallet WHERE id = $1 FOR UPDATE"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Find the wallet owned by a roster person, if any.
pub async fn find_by_club_player<'e>(
    executor: impl PgExecutor<'e>,
    club_player_id: Uuid,
) -> SqlxResult<Option<DrinkWalletRow>> {
    sqlx::query_as::<_, DrinkWalletRow>(&format!(
        "SELECT {COLUMNS} FROM drink_wallet WHERE club_player_id = $1"
    ))
    .bind(club_player_id)
    .fetch_optional(executor)
    .await
}

/// Create a wallet. `club_player_id` None creates a bearer (anonymous) wallet.
pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    club_player_id: Option<Uuid>,
) -> SqlxResult<DrinkWalletRow> {
    sqlx::query_as::<_, DrinkWalletRow>(&format!(
        "INSERT INTO drink_wallet (club_id, club_player_id) \
         VALUES ($1, $2) RETURNING {COLUMNS}"
    ))
    .bind(club_id)
    .bind(club_player_id)
    .fetch_one(executor)
    .await
}

/// Apply a delta to the cached balance and return the updated row. The DB
/// `CHECK (balance >= 0)` is the backstop against overspend. Callers must hold the
/// wallet's `FOR UPDATE` lock for the delta to be race-free.
pub async fn apply_balance_delta<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    delta: i32,
) -> SqlxResult<DrinkWalletRow> {
    sqlx::query_as::<_, DrinkWalletRow>(&format!(
        "UPDATE drink_wallet SET balance = balance + $2 WHERE id = $1 RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(delta)
    .fetch_one(executor)
    .await
}

/// Bind a bearer wallet to a roster person (gives it an owner). Does not move any
/// balance. Fails the UNIQUE constraint if that person already owns a wallet.
pub async fn set_owner<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    club_player_id: Uuid,
) -> SqlxResult<DrinkWalletRow> {
    sqlx::query_as::<_, DrinkWalletRow>(&format!(
        "UPDATE drink_wallet SET club_player_id = $2 WHERE id = $1 RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(club_player_id)
    .fetch_one(executor)
    .await
}
