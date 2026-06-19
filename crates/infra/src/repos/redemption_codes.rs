use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::RedemptionCodeRow;

const COLS: &str =
    "id, code, plan, trial_days, max_uses, used_count, expires_at, note, created_at, updated_at";

/// Unambiguous alphabet for generated codes (no I/O/0/1 — easy to read aloud
/// and type). 32 symbols, so 8 random chars ≈ 10^12 of space.
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Generate a fresh OTP-style code like `PPF86LH9PC` (stored without
/// separators; presented to users as `PP-XXXX-XXXX`). Randomness comes from a
/// v4 UUID so we avoid pulling in an extra RNG dependency. Callers should
/// retry on the (astronomically unlikely) unique-constraint collision.
pub fn generate_code() -> String {
    let bytes = Uuid::new_v4().into_bytes();
    let body: String = bytes
        .iter()
        .take(8)
        .map(|b| CODE_ALPHABET[(*b as usize) % CODE_ALPHABET.len()] as char)
        .collect();
    format!("PP{body}")
}

/// Look up a code and lock its row `FOR UPDATE` so two clubs racing on the last
/// remaining use of a capped code are serialized. Must run inside a transaction
/// to hold the lock until commit. Code is matched verbatim — normalize
/// (trim + upper-case) before calling.
pub async fn lock_by_code<'e>(
    executor: impl PgExecutor<'e>,
    code: &str,
) -> SqlxResult<Option<RedemptionCodeRow>> {
    sqlx::query_as::<_, RedemptionCodeRow>(&format!(
        "SELECT {COLS} FROM redemption_codes WHERE code = $1 FOR UPDATE"
    ))
    .bind(code)
    .fetch_optional(executor)
    .await
}

/// Whether this club has already redeemed this code.
pub async fn has_used<'e>(
    executor: impl PgExecutor<'e>,
    code_id: Uuid,
    club_id: Uuid,
) -> SqlxResult<bool> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM redemption_code_uses WHERE code_id = $1 AND club_id = $2)",
    )
    .bind(code_id)
    .bind(club_id)
    .fetch_one(executor)
    .await
}

/// Record a club's redemption of a code. Relies on the `(code_id, club_id)`
/// unique constraint to reject a double-redeem at the DB level.
pub async fn insert_use<'e>(
    executor: impl PgExecutor<'e>,
    code_id: Uuid,
    club_id: Uuid,
    redeemed_by: Uuid,
) -> SqlxResult<()> {
    sqlx::query(
        "INSERT INTO redemption_code_uses (code_id, club_id, redeemed_by) VALUES ($1, $2, $3)",
    )
    .bind(code_id)
    .bind(club_id)
    .bind(redeemed_by)
    .execute(executor)
    .await?;
    Ok(())
}

/// Bump the running redemption count after a successful use.
pub async fn increment_used<'e>(executor: impl PgExecutor<'e>, code_id: Uuid) -> SqlxResult<()> {
    sqlx::query("UPDATE redemption_codes SET used_count = used_count + 1 WHERE id = $1")
        .bind(code_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// List the most recently created codes (admin overview), newest first.
pub async fn list_recent<'e>(
    executor: impl PgExecutor<'e>,
    limit: i64,
) -> SqlxResult<Vec<RedemptionCodeRow>> {
    sqlx::query_as::<_, RedemptionCodeRow>(&format!(
        "SELECT {COLS} FROM redemption_codes ORDER BY created_at DESC LIMIT $1"
    ))
    .bind(limit)
    .fetch_all(executor)
    .await
}

/// Mint a new code (admin only). `code` should already be normalized.
pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    code: &str,
    plan: &str,
    trial_days: i32,
    max_uses: Option<i32>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    note: Option<&str>,
) -> SqlxResult<RedemptionCodeRow> {
    sqlx::query_as::<_, RedemptionCodeRow>(&format!(
        "INSERT INTO redemption_codes (code, plan, trial_days, max_uses, expires_at, note) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING {COLS}"
    ))
    .bind(code)
    .bind(plan)
    .bind(trial_days)
    .bind(max_uses)
    .bind(expires_at)
    .bind(note)
    .fetch_one(executor)
    .await
}
