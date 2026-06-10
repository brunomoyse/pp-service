use async_graphql::dataloader::DataLoader;
use async_graphql::{ComplexObject, Context, Enum, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::gql::error::ResultExt;
use crate::gql::loaders::DrinkLedgerLoader;

use infra::models::{BarStationRow, DrinkLedgerEntryRow, DrinkRedemptionRow, DrinkWalletRow};

/// Why a ledger entry exists.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum DrinkLedgerReason {
    TournamentTopup,
    BarRedemption,
    Expiry,
    Adjustment,
    Transfer,
}

impl From<String> for DrinkLedgerReason {
    fn from(value: String) -> Self {
        match value.as_str() {
            "tournament_topup" => DrinkLedgerReason::TournamentTopup,
            "bar_redemption" => DrinkLedgerReason::BarRedemption,
            "expiry" => DrinkLedgerReason::Expiry,
            "transfer" => DrinkLedgerReason::Transfer,
            _ => DrinkLedgerReason::Adjustment,
        }
    }
}

/// A drink wallet's cached balance plus, on request, its recent ledger.
#[derive(SimpleObject, Clone)]
#[graphql(complex)]
pub struct DrinkWallet {
    pub id: ID,
    pub club_id: ID,
    /// None for a bearer (anonymous) wallet.
    pub club_player_id: Option<ID>,
    /// Server-authoritative cached balance (= SUM of ledger deltas).
    pub balance: i32,
    pub created_at: DateTime<Utc>,
}

impl From<DrinkWalletRow> for DrinkWallet {
    fn from(row: DrinkWalletRow) -> Self {
        Self {
            id: row.id.into(),
            club_id: row.club_id.into(),
            club_player_id: row.club_player_id.map(Into::into),
            balance: row.balance,
            created_at: row.created_at,
        }
    }
}

#[ComplexObject]
impl DrinkWallet {
    /// The most recent ledger entries for display (newest first, capped).
    async fn recent_entries(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<DrinkLedgerEntry>> {
        let wallet_id = Uuid::parse_str(self.id.as_str()).gql_err("Invalid wallet ID")?;
        let loader = ctx.data::<DataLoader<DrinkLedgerLoader>>()?;
        let entries = loader
            .load_one(wallet_id)
            .await
            .gql_err("Loading ledger failed")?
            .unwrap_or_default();
        Ok(entries.into_iter().map(DrinkLedgerEntry::from).collect())
    }
}

/// One append-only ledger movement.
#[derive(SimpleObject, Clone)]
pub struct DrinkLedgerEntry {
    pub id: ID,
    pub wallet_id: ID,
    pub delta: i32,
    pub reason: DrinkLedgerReason,
    pub tournament_id: Option<ID>,
    pub expires_at: Option<DateTime<Utc>>,
    pub redemption_id: Option<ID>,
    pub created_at: DateTime<Utc>,
}

impl From<DrinkLedgerEntryRow> for DrinkLedgerEntry {
    fn from(row: DrinkLedgerEntryRow) -> Self {
        Self {
            id: row.id.into(),
            wallet_id: row.wallet_id.into(),
            delta: row.delta,
            reason: row.reason.into(),
            tournament_id: row.tournament_id.map(Into::into),
            expires_at: row.expires_at,
            redemption_id: row.redemption_id.map(Into::into),
            created_at: row.created_at,
        }
    }
}

/// One drink served at the bar.
#[derive(SimpleObject, Clone)]
pub struct DrinkRedemption {
    pub id: ID,
    pub wallet_id: ID,
    pub bar_station_id: ID,
    pub drink_type: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<DrinkRedemptionRow> for DrinkRedemption {
    fn from(row: DrinkRedemptionRow) -> Self {
        Self {
            id: row.id.into(),
            wallet_id: row.wallet_id.into(),
            bar_station_id: row.bar_station_id.into(),
            drink_type: row.drink_type,
            created_at: row.created_at,
        }
    }
}

/// A club-scoped point of redemption.
#[derive(SimpleObject, Clone)]
pub struct BarStation {
    pub id: ID,
    pub club_id: ID,
    pub name: String,
    pub is_active: bool,
}

impl From<BarStationRow> for BarStation {
    fn from(row: BarStationRow) -> Self {
        Self {
            id: row.id.into(),
            club_id: row.club_id.into(),
            name: row.name,
            is_active: row.is_active,
        }
    }
}

/// A freshly generated printed card. `token` is returned exactly once — render it
/// into a QR; it is never recoverable afterwards.
#[derive(SimpleObject, Clone)]
pub struct DrinkCard {
    pub credential_id: ID,
    pub token: String,
}

// ---- Inputs ----

#[derive(InputObject)]
pub struct CreateBarStationInput {
    pub club_id: ID,
    pub name: String,
}

#[derive(InputObject)]
pub struct GenerateDrinkCardsInput {
    /// The club whose manager is generating the cards (authorization scope).
    pub club_id: ID,
    pub count: i32,
}

#[derive(InputObject)]
pub struct RedeemDrinkInput {
    /// The raw secret scanned from the player's QR / pass / printed card.
    pub credential_token: String,
    pub bar_station_id: ID,
    /// Caller-supplied key that makes a retried scan debit at most once.
    pub idempotency_key: String,
    pub drink_type: Option<String>,
}

#[derive(InputObject)]
pub struct TopUpWalletInput {
    pub wallet_id: ID,
    pub amount: i32,
    pub tournament_id: Option<ID>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(InputObject)]
pub struct ActivatePrintedCardInput {
    pub club_id: ID,
    pub credential_token: String,
    /// Some(name) makes a named wallet; omit to leave the card bearer.
    pub display_name: Option<String>,
    pub initial_top_up: Option<i32>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(InputObject)]
pub struct ClaimCardInput {
    pub credential_token: String,
}

// ---- Payloads ----

#[derive(SimpleObject)]
pub struct RedeemDrinkPayload {
    pub wallet_id: ID,
    pub balance: i32,
    pub redemption: DrinkRedemption,
    /// True when this matched a prior redemption (retried scan) and did not debit again.
    pub deduped: bool,
}

#[derive(SimpleObject)]
pub struct TopUpWalletPayload {
    pub wallet_id: ID,
    pub balance: i32,
    pub ledger_entry: DrinkLedgerEntry,
}

#[derive(SimpleObject)]
pub struct ActivatePrintedCardPayload {
    pub wallet: DrinkWallet,
}

#[derive(SimpleObject)]
pub struct ClaimCardPayload {
    pub wallet: DrinkWallet,
    pub message: String,
}

#[derive(SimpleObject)]
pub struct GenerateDrinkCardsPayload {
    pub cards: Vec<DrinkCard>,
}
