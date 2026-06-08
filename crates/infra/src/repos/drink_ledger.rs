use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::DrinkLedgerEntryRow;

const COLUMNS: &str = "id, wallet_id, delta, reason, tournament_id, expires_at, \
                       redemption_id, source_ledger_entry_id, transfer_id, created_by, created_at";

/// All the fields needed to append one ledger entry. Construct with `NewLedgerEntry`
/// helpers for the common shapes.
#[derive(Debug, Clone)]
pub struct NewLedgerEntry {
    pub wallet_id: Uuid,
    pub delta: i32,
    pub reason: &'static str,
    pub tournament_id: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
    pub redemption_id: Option<Uuid>,
    pub source_ledger_entry_id: Option<Uuid>,
    pub transfer_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
}

impl NewLedgerEntry {
    /// A credit top-up lot.
    pub fn topup(
        wallet_id: Uuid,
        amount: i32,
        reason: &'static str,
        tournament_id: Option<Uuid>,
        expires_at: Option<DateTime<Utc>>,
        created_by: Option<Uuid>,
    ) -> Self {
        Self {
            wallet_id,
            delta: amount,
            reason,
            tournament_id,
            expires_at,
            redemption_id: None,
            source_ledger_entry_id: None,
            transfer_id: None,
            created_by,
        }
    }

    /// The `-1` entry for a bar redemption.
    pub fn redemption(wallet_id: Uuid, redemption_id: Uuid, created_by: Option<Uuid>) -> Self {
        Self {
            wallet_id,
            delta: -1,
            reason: "bar_redemption",
            tournament_id: None,
            expires_at: None,
            redemption_id: Some(redemption_id),
            source_ledger_entry_id: None,
            transfer_id: None,
            created_by,
        }
    }

    /// A negative entry expiring the unconsumed remainder of a specific lot.
    pub fn expiry(wallet_id: Uuid, amount: i32, source_lot_id: Uuid) -> Self {
        Self {
            wallet_id,
            delta: -amount,
            reason: "expiry",
            tournament_id: None,
            expires_at: None,
            redemption_id: None,
            source_ledger_entry_id: Some(source_lot_id),
            transfer_id: None,
            created_by: None,
        }
    }
}

/// Append one ledger entry. The ledger is append-only — never UPDATE/DELETE.
pub async fn insert<'e>(
    executor: impl PgExecutor<'e>,
    entry: NewLedgerEntry,
) -> SqlxResult<DrinkLedgerEntryRow> {
    sqlx::query_as::<_, DrinkLedgerEntryRow>(&format!(
        "INSERT INTO drink_ledger_entry \
         (wallet_id, delta, reason, tournament_id, expires_at, redemption_id, \
          source_ledger_entry_id, transfer_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING {COLUMNS}"
    ))
    .bind(entry.wallet_id)
    .bind(entry.delta)
    .bind(entry.reason)
    .bind(entry.tournament_id)
    .bind(entry.expires_at)
    .bind(entry.redemption_id)
    .bind(entry.source_ledger_entry_id)
    .bind(entry.transfer_id)
    .bind(entry.created_by)
    .fetch_one(executor)
    .await
}

/// Recent ledger entries for a wallet (newest first), for display.
pub async fn list_recent_by_wallet<'e>(
    executor: impl PgExecutor<'e>,
    wallet_id: Uuid,
    limit: i64,
) -> SqlxResult<Vec<DrinkLedgerEntryRow>> {
    sqlx::query_as::<_, DrinkLedgerEntryRow>(&format!(
        "SELECT {COLUMNS} FROM drink_ledger_entry \
         WHERE wallet_id = $1 ORDER BY created_at DESC, id DESC LIMIT $2"
    ))
    .bind(wallet_id)
    .bind(limit)
    .fetch_all(executor)
    .await
}

/// The full ledger for a wallet in chronological order. Used by the expiry job to
/// replay FIFO consumption.
pub async fn list_all_by_wallet<'e>(
    executor: impl PgExecutor<'e>,
    wallet_id: Uuid,
) -> SqlxResult<Vec<DrinkLedgerEntryRow>> {
    sqlx::query_as::<_, DrinkLedgerEntryRow>(&format!(
        "SELECT {COLUMNS} FROM drink_ledger_entry \
         WHERE wallet_id = $1 ORDER BY created_at ASC, id ASC"
    ))
    .bind(wallet_id)
    .fetch_all(executor)
    .await
}

/// Wallet ids that have at least one positive lot whose expiry has passed — the
/// candidates the nightly expiry job must inspect.
pub async fn wallets_with_expired_lots<'e>(
    executor: impl PgExecutor<'e>,
    now: DateTime<Utc>,
) -> SqlxResult<Vec<Uuid>> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT DISTINCT wallet_id FROM drink_ledger_entry \
         WHERE delta > 0 AND expires_at IS NOT NULL AND expires_at <= $1",
    )
    .bind(now)
    .fetch_all(executor)
    .await
}
