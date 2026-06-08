use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::DrinkWalletCredentialRow;

// `type` is selected last and mapped onto `credential_type` via the model's
// `#[sqlx(rename = "type")]`.
const COLUMNS: &str = "id, wallet_id, token_hash, status, created_at, updated_at, type";

/// Create a printed-card credential: status `printed`, no wallet yet. The raw token
/// (kept only in the QR) is hashed by the caller; we store just the hash.
pub async fn create_printed_card<'e>(
    executor: impl PgExecutor<'e>,
    token_hash: &[u8],
) -> SqlxResult<DrinkWalletCredentialRow> {
    sqlx::query_as::<_, DrinkWalletCredentialRow>(&format!(
        "INSERT INTO drink_wallet_credential (type, token_hash, status) \
         VALUES ('printed_card', $1, 'printed') RETURNING {COLUMNS}"
    ))
    .bind(token_hash)
    .fetch_one(executor)
    .await
}

/// Resolve an active credential by its token hash (any type). The entry point for
/// redemption and claim. Revoked/printed/consumed credentials do not match.
pub async fn find_active_by_hash<'e>(
    executor: impl PgExecutor<'e>,
    token_hash: &[u8],
) -> SqlxResult<Option<DrinkWalletCredentialRow>> {
    sqlx::query_as::<_, DrinkWalletCredentialRow>(&format!(
        "SELECT {COLUMNS} FROM drink_wallet_credential \
         WHERE token_hash = $1 AND status = 'active'"
    ))
    .bind(token_hash)
    .fetch_optional(executor)
    .await
}

/// Resolve an unassigned printed card by its token hash, for activation.
pub async fn find_printed_card_by_hash<'e>(
    executor: impl PgExecutor<'e>,
    token_hash: &[u8],
) -> SqlxResult<Option<DrinkWalletCredentialRow>> {
    sqlx::query_as::<_, DrinkWalletCredentialRow>(&format!(
        "SELECT {COLUMNS} FROM drink_wallet_credential \
         WHERE token_hash = $1 AND type = 'printed_card' AND status = 'printed'"
    ))
    .bind(token_hash)
    .fetch_optional(executor)
    .await
}

/// Bind a printed card to a wallet and activate it.
pub async fn bind_to_wallet<'e>(
    executor: impl PgExecutor<'e>,
    credential_id: Uuid,
    wallet_id: Uuid,
) -> SqlxResult<DrinkWalletCredentialRow> {
    sqlx::query_as::<_, DrinkWalletCredentialRow>(&format!(
        "UPDATE drink_wallet_credential \
         SET wallet_id = $2, status = 'active' \
         WHERE id = $1 RETURNING {COLUMNS}"
    ))
    .bind(credential_id)
    .bind(wallet_id)
    .fetch_one(executor)
    .await
}

/// Revoke a credential so it can no longer redeem (independently of the wallet).
pub async fn revoke<'e>(
    executor: impl PgExecutor<'e>,
    credential_id: Uuid,
) -> SqlxResult<Option<DrinkWalletCredentialRow>> {
    sqlx::query_as::<_, DrinkWalletCredentialRow>(&format!(
        "UPDATE drink_wallet_credential SET status = 'revoked' \
         WHERE id = $1 RETURNING {COLUMNS}"
    ))
    .bind(credential_id)
    .fetch_optional(executor)
    .await
}
