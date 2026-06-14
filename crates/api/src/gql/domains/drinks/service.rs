//! Transport-agnostic business logic for the drink-voucher wallet.
//!
//! Resolvers handle auth, ID parsing, and GraphQL conversions; everything here owns
//! the database transaction and the correctness-critical invariants (row locking,
//! idempotency, append-only ledger, cached balance = SUM(delta)).

use chrono::{DateTime, Utc};
use rand::distr::Alphanumeric;
use rand::RngExt;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use infra::models::{DrinkLedgerEntryRow, DrinkRedemptionRow, DrinkWalletRow};
use infra::repos::{
    bar_stations, club_players, drink_ledger, drink_ledger::NewLedgerEntry, drink_redemptions,
    drink_wallet_credentials, drink_wallets,
};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Generate a high-entropy credential secret. 32 alphanumeric characters carry well
/// over 128 bits of entropy; the raw value lives only in the QR, never in the DB.
pub fn generate_credential_token() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

/// SHA-256 of a credential secret, stored as the `token_hash` bytea. Mirrors the
/// existing refresh-token hashing approach. Treat the raw token like a bearer token.
pub fn hash_credential_token(raw: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hasher.finalize().iter().copied().collect()
}

// ---- Printed card generation ----

/// A freshly generated printed card: the credential id plus its raw secret, which is
/// returned exactly once (to be rendered into a QR) and never recoverable afterwards.
pub struct GeneratedCard {
    pub credential_id: Uuid,
    pub token: String,
}

/// Generate `count` blank printed-card credentials (`status='printed'`, no wallet).
pub async fn generate_printed_cards(
    pool: &sqlx::PgPool,
    count: i32,
) -> Result<Vec<GeneratedCard>, BoxError> {
    if count <= 0 {
        return Err("count must be positive".into());
    }
    if count > 500 {
        return Err("cannot generate more than 500 cards at once".into());
    }

    let mut tx = pool.begin().await?;
    let mut cards = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let token = generate_credential_token();
        let hash = hash_credential_token(&token);
        let credential = drink_wallet_credentials::create_printed_card(&mut *tx, &hash).await?;
        cards.push(GeneratedCard {
            credential_id: credential.id,
            token,
        });
    }
    tx.commit().await?;
    Ok(cards)
}

// ---- Redemption (correctness-critical) ----

pub struct RedeemParams {
    pub raw_token: String,
    pub bar_station_id: Uuid,
    pub idempotency_key: String,
    pub drink_type: Option<String>,
    pub operator_user_id: Uuid,
}

pub struct RedeemOutcome {
    pub wallet_id: Uuid,
    pub balance: i32,
    pub redemption: DrinkRedemptionRow,
    /// True when this call matched a prior redemption (a retried scan) and did not
    /// debit again.
    pub deduped: bool,
}

/// Redeem one drink. In a single transaction: resolve the credential, lock the
/// wallet row, dedupe on the idempotency key, reject on insufficient balance, then
/// insert the redemption + a `-1` ledger entry and decrement the cached balance.
///
/// Safe under concurrent scans (the `FOR UPDATE` lock serialises debits on a wallet)
/// and idempotent on `idempotency_key` (no double-debit on retry).
pub async fn redeem_drink(
    pool: &sqlx::PgPool,
    params: RedeemParams,
) -> Result<RedeemOutcome, BoxError> {
    let hash = hash_credential_token(&params.raw_token);

    let mut tx = pool.begin().await?;

    // 1. Resolve the credential (must be active) -> wallet.
    let credential = drink_wallet_credentials::find_active_by_hash(&mut *tx, &hash)
        .await?
        .ok_or("Invalid or inactive drink credential")?;
    let wallet_id = credential
        .wallet_id
        .ok_or("This card is not linked to a wallet")?;

    // 2. Lock the wallet row so the balance read-check-write is race-free.
    let wallet = drink_wallets::get_by_id_for_update(&mut *tx, wallet_id)
        .await?
        .ok_or("Wallet not found")?;

    // Validate the bar station: it must belong to the wallet's club and be active.
    let station = bar_stations::get_by_id(&mut *tx, params.bar_station_id)
        .await?
        .ok_or("Bar station not found")?;
    if station.club_id != wallet.club_id {
        return Err("Bar station belongs to a different club".into());
    }
    if !station.is_active {
        return Err("Bar station is not active".into());
    }

    // 3. Idempotency: a retried scan resolves to the original redemption, no debit.
    if let Some(existing) =
        drink_redemptions::find_by_idempotency(&mut *tx, wallet_id, &params.idempotency_key).await?
    {
        tx.commit().await?;
        return Ok(RedeemOutcome {
            wallet_id,
            balance: wallet.balance,
            redemption: existing,
            deduped: true,
        });
    }

    // 4. Reject if there is nothing to spend.
    if wallet.balance < 1 {
        return Err("Insufficient drink balance".into());
    }

    // 5. Insert the redemption. The UNIQUE(wallet_id, idempotency_key) constraint is
    //    the ultimate guard; on a genuinely simultaneous retry it raises a unique
    //    violation, which we resolve to the original redemption (still no double-debit).
    let redemption = match drink_redemptions::create(
        &mut *tx,
        wallet_id,
        params.bar_station_id,
        params.drink_type.as_deref(),
        &params.idempotency_key,
        Some(params.operator_user_id),
    )
    .await
    {
        Ok(row) => row,
        Err(e) => {
            if matches!(&e, sqlx::Error::Database(db) if db.is_unique_violation()) {
                // Roll back our half-done work and return the winning redemption.
                drop(tx);
                let existing = drink_redemptions::find_by_idempotency(
                    pool,
                    wallet_id,
                    &params.idempotency_key,
                )
                .await?
                .ok_or("Redemption conflict could not be resolved")?;
                let current = drink_wallets::get_by_id(pool, wallet_id)
                    .await?
                    .ok_or("Wallet not found")?;
                return Ok(RedeemOutcome {
                    wallet_id,
                    balance: current.balance,
                    redemption: existing,
                    deduped: true,
                });
            }
            return Err(e.into());
        }
    };

    // 6. Append the -1 ledger entry and decrement the cached balance. The DB
    //    CHECK (balance >= 0) backstops the application-level balance check.
    drink_ledger::insert(
        &mut *tx,
        NewLedgerEntry::redemption(wallet_id, redemption.id, Some(params.operator_user_id)),
    )
    .await?;
    let updated = drink_wallets::apply_balance_delta(&mut *tx, wallet_id, -1).await?;

    tx.commit().await?;

    Ok(RedeemOutcome {
        wallet_id,
        balance: updated.balance,
        redemption,
        deduped: false,
    })
}

// ---- Top-up ----

pub struct TopUpParams {
    pub wallet_id: Uuid,
    pub amount: i32,
    pub tournament_id: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
    pub operator_user_id: Uuid,
}

pub struct TopUpOutcome {
    pub wallet_id: Uuid,
    pub balance: i32,
    pub ledger_entry: DrinkLedgerEntryRow,
}

/// Add credits to a wallet: append a positive ledger lot and bump the cached balance.
pub async fn top_up_wallet(
    pool: &sqlx::PgPool,
    params: TopUpParams,
) -> Result<TopUpOutcome, BoxError> {
    if params.amount <= 0 {
        return Err("amount must be positive".into());
    }

    let mut tx = pool.begin().await?;

    let _wallet = drink_wallets::get_by_id_for_update(&mut *tx, params.wallet_id)
        .await?
        .ok_or("Wallet not found")?;

    // A top-up tied to a tournament is a registration credit; otherwise a manual
    // adjustment.
    let reason = if params.tournament_id.is_some() {
        "tournament_topup"
    } else {
        "adjustment"
    };

    let ledger_entry = drink_ledger::insert(
        &mut *tx,
        NewLedgerEntry::topup(
            params.wallet_id,
            params.amount,
            reason,
            params.tournament_id,
            params.expires_at,
            Some(params.operator_user_id),
        ),
    )
    .await?;

    let updated =
        drink_wallets::apply_balance_delta(&mut *tx, params.wallet_id, params.amount).await?;

    tx.commit().await?;

    Ok(TopUpOutcome {
        wallet_id: params.wallet_id,
        balance: updated.balance,
        ledger_entry,
    })
}

// ---- Printed card activation ----

pub struct ActivateParams {
    pub raw_token: String,
    pub club_id: Uuid,
    /// Some(name) creates a named wallet via a new roster person; None leaves it bearer.
    pub display_name: Option<String>,
    pub initial_top_up: Option<i32>,
    pub expires_at: Option<DateTime<Utc>>,
    pub operator_user_id: Uuid,
}

pub struct ActivateOutcome {
    pub wallet: DrinkWalletRow,
}

/// Activate an unassigned printed card: create a wallet (named or bearer), bind the
/// credential, and optionally seed an initial balance.
pub async fn activate_printed_card(
    pool: &sqlx::PgPool,
    params: ActivateParams,
) -> Result<ActivateOutcome, BoxError> {
    let hash = hash_credential_token(&params.raw_token);

    let mut tx = pool.begin().await?;

    let credential = drink_wallet_credentials::find_printed_card_by_hash(&mut *tx, &hash)
        .await?
        .ok_or("Card not found or already activated")?;

    // Optionally create a roster person to make this a named wallet.
    let club_player_id = match params.display_name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => Some(
            club_players::create(&mut *tx, params.club_id, name, None, None, None)
                .await?
                .id,
        ),
        _ => None,
    };

    let wallet = drink_wallets::create(&mut *tx, params.club_id, club_player_id).await?;
    drink_wallet_credentials::bind_to_wallet(&mut *tx, credential.id, wallet.id).await?;

    if let Some(amount) = params.initial_top_up {
        if amount > 0 {
            drink_ledger::insert(
                &mut *tx,
                NewLedgerEntry::topup(
                    wallet.id,
                    amount,
                    "adjustment",
                    None,
                    params.expires_at,
                    Some(params.operator_user_id),
                ),
            )
            .await?;
            drink_wallets::apply_balance_delta(&mut *tx, wallet.id, amount).await?;
        }
    }

    // Re-read so the returned balance reflects any initial top-up.
    let final_wallet = drink_wallets::get_by_id(&mut *tx, wallet.id)
        .await?
        .ok_or("Wallet vanished after creation")?;

    tx.commit().await?;

    Ok(ActivateOutcome {
        wallet: final_wallet,
    })
}

// ---- Claim (player self-service) ----

pub struct ClaimParams {
    pub raw_token: String,
    pub app_user_id: Uuid,
    /// Used as the roster display name when claiming a bearer card.
    pub display_name: String,
}

pub struct ClaimOutcome {
    pub wallet: DrinkWalletRow,
    pub message: String,
}

/// Attach an owner (the calling player) to the wallet a card already points at.
/// This never moves any balance — it is the same wallet, it just gains an owner.
pub async fn claim_card(
    pool: &sqlx::PgPool,
    params: ClaimParams,
) -> Result<ClaimOutcome, BoxError> {
    let hash = hash_credential_token(&params.raw_token);

    let mut tx = pool.begin().await?;

    let credential = drink_wallet_credentials::find_active_by_hash(&mut *tx, &hash)
        .await?
        .ok_or("Card not found or not active")?;
    let wallet_id = credential
        .wallet_id
        .ok_or("This card has not been activated yet")?;

    let wallet = drink_wallets::get_by_id_for_update(&mut *tx, wallet_id)
        .await?
        .ok_or("Wallet not found")?;

    let message = match wallet.club_player_id {
        Some(rp_id) => {
            let roster = club_players::get_by_id(&mut *tx, rp_id)
                .await?
                .ok_or("Roster entry missing")?;
            match roster.app_user_id {
                Some(uid) if uid == params.app_user_id => {
                    "Card already linked to your account".to_string()
                }
                Some(_) => {
                    return Err("This card is already owned by another account".into());
                }
                None => {
                    club_players::claim(&mut *tx, rp_id, params.app_user_id)
                        .await?
                        .ok_or("Failed to claim roster entry")?;
                    "Card linked to your account".to_string()
                }
            }
        }
        None => {
            // Bearer card: find or create the caller's roster entry in this club, then
            // bind the wallet to it.
            let roster = match club_players::find_by_club_and_app_user(
                &mut *tx,
                wallet.club_id,
                params.app_user_id,
            )
            .await?
            {
                Some(rp) => rp,
                None => {
                    club_players::create(
                        &mut *tx,
                        wallet.club_id,
                        &params.display_name,
                        None,
                        None,
                        Some(params.app_user_id),
                    )
                    .await?
                }
            };

            // The genuine two-wallet case (Phase 3 merge) is out of scope: refuse
            // rather than silently stranding a balance.
            if let Some(existing) = drink_wallets::find_by_club_player(&mut *tx, roster.id).await? {
                if existing.id != wallet.id {
                    return Err(
                        "You already have a drink wallet at this club; merging wallets is not supported yet"
                            .into(),
                    );
                }
            }

            drink_wallets::set_owner(&mut *tx, wallet.id, roster.id).await?;
            "Card linked to your account".to_string()
        }
    };

    let final_wallet = drink_wallets::get_by_id(&mut *tx, wallet_id)
        .await?
        .ok_or("Wallet vanished")?;

    tx.commit().await?;

    Ok(ClaimOutcome {
        wallet: final_wallet,
        message,
    })
}

// ---- Expiry (nightly job) ----

/// Replay a wallet's ledger chronologically with FIFO consumption and return the
/// `(lot_id, amount)` of each expired lot's unconsumed remainder.
///
/// Rules: a positive entry opens a credit lot. A non-expiry debit (redemption,
/// transfer, negative adjustment) consumes open lots oldest-first. An `expiry` entry
/// consumes only the specific lot it references — that pin is what makes the job
/// re-run safe, since a re-run replays the prior expiry against the same lot and
/// finds nothing left to expire.
pub fn compute_expiries(entries: &[DrinkLedgerEntryRow], now: DateTime<Utc>) -> Vec<(Uuid, i32)> {
    struct Lot {
        id: Uuid,
        remaining: i64,
        expires_at: Option<DateTime<Utc>>,
    }

    let mut lots: Vec<Lot> = Vec::new();

    for entry in entries {
        if entry.delta > 0 {
            lots.push(Lot {
                id: entry.id,
                remaining: entry.delta as i64,
                expires_at: entry.expires_at,
            });
        } else {
            let mut need = -(entry.delta as i64);
            if entry.reason == "expiry" {
                if let Some(src) = entry.source_ledger_entry_id {
                    if let Some(lot) = lots.iter_mut().find(|l| l.id == src) {
                        let take = need.min(lot.remaining);
                        lot.remaining -= take;
                    }
                }
            } else {
                for lot in lots.iter_mut() {
                    if need == 0 {
                        break;
                    }
                    let take = need.min(lot.remaining);
                    lot.remaining -= take;
                    need -= take;
                }
            }
        }
    }

    lots.into_iter()
        .filter(|lot| lot.remaining > 0 && matches!(lot.expires_at, Some(t) if t <= now))
        .map(|lot| (lot.id, lot.remaining as i32))
        .collect()
}

/// Expire one wallet's overdue lots inside a locked transaction. Returns the total
/// number of credits expired.
async fn expire_wallet(
    pool: &sqlx::PgPool,
    wallet_id: Uuid,
    now: DateTime<Utc>,
) -> Result<i32, BoxError> {
    let mut tx = pool.begin().await?;

    if drink_wallets::get_by_id_for_update(&mut *tx, wallet_id)
        .await?
        .is_none()
    {
        tx.commit().await?;
        return Ok(0);
    }

    let entries = drink_ledger::list_all_by_wallet(&mut *tx, wallet_id).await?;
    let expiries = compute_expiries(&entries, now);

    let mut total = 0i32;
    for (lot_id, amount) in expiries {
        drink_ledger::insert(&mut *tx, NewLedgerEntry::expiry(wallet_id, amount, lot_id)).await?;
        total += amount;
    }
    if total > 0 {
        drink_wallets::apply_balance_delta(&mut *tx, wallet_id, -total).await?;
    }

    tx.commit().await?;
    Ok(total)
}

/// Expire all overdue credits across every wallet with expired lots. Returns the
/// total number of credits expired. Safe to re-run.
pub async fn run_expiry(pool: &sqlx::PgPool, now: DateTime<Utc>) -> Result<i32, BoxError> {
    let wallet_ids = drink_ledger::wallets_with_expired_lots(pool, now).await?;
    let mut total = 0i32;
    for wallet_id in wallet_ids {
        total += expire_wallet(pool, wallet_id, now).await?;
    }
    Ok(total)
}
