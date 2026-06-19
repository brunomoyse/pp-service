use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::ClubRow;

/// Fields needed to create a club via self-serve onboarding.
pub struct CreateClubData {
    pub name: String,
    pub address: Option<String>,
    pub city: Option<String>,
    pub postal_code: Option<String>,
    pub country: String,
    pub vat_number: Option<String>,
    /// Flagged for manual review when the company isn't a verified non-profit.
    pub needs_review: bool,
    /// Billing tier at creation: "free" | "club". ("casino" is sales-led, not
    /// self-serve.)
    pub plan: String,
}

pub async fn list<'e>(executor: impl PgExecutor<'e>) -> SqlxResult<Vec<ClubRow>> {
    sqlx::query_as::<_, ClubRow>(
        r#"
        SELECT id, name, city, postal_code, province, country, address, vat_number, needs_review, plan, subscription_status, subscription_expires_at, created_at, updated_at
        FROM clubs
        ORDER BY name ASC
        "#,
    )
    .fetch_all(executor)
    .await
}

/// Distinct, non-null province slugs that at least one club resolves to —
/// the set a province leaderboard filter can offer.
pub async fn list_provinces<'e>(executor: impl PgExecutor<'e>) -> SqlxResult<Vec<String>> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT DISTINCT province
        FROM clubs
        WHERE province IS NOT NULL AND plan <> 'free'
        ORDER BY province ASC
        "#,
    )
    .fetch_all(executor)
    .await
}

/// Whether a club is on the free ("Home Game") tier — used to hide it from the
/// player app. Missing club resolves to `false`.
pub async fn is_free<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> SqlxResult<bool> {
    let plan: Option<String> = sqlx::query_scalar("SELECT plan FROM clubs WHERE id = $1")
        .bind(id)
        .fetch_optional(executor)
        .await?;
    Ok(plan.as_deref() == Some("free"))
}

pub async fn get_by_id<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> SqlxResult<Option<ClubRow>> {
    sqlx::query_as::<_, ClubRow>(
        r#"
        SELECT id, name, city, postal_code, province, country, address, vat_number, needs_review, plan, subscription_status, subscription_expires_at, created_at, updated_at
        FROM clubs
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Insert a club. Generic over the executor so it can run inside the
/// onboarding transaction alongside the user + club-manager inserts.
pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    data: CreateClubData,
) -> SqlxResult<ClubRow> {
    sqlx::query_as::<_, ClubRow>(
        r#"
        INSERT INTO clubs (name, address, city, postal_code, country, vat_number, needs_review, plan)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id, name, city, postal_code, province, country, address, vat_number, needs_review, plan, subscription_status, subscription_expires_at, created_at, updated_at
        "#,
    )
    .bind(&data.name)
    .bind(&data.address)
    .bind(&data.city)
    .bind(&data.postal_code)
    .bind(&data.country)
    .bind(&data.vat_number)
    .bind(data.needs_review)
    .bind(&data.plan)
    .fetch_one(executor)
    .await
}

/// Set a club's plan + subscription lifecycle fields. Called by the payments
/// service (via an admin-guarded mutation) when a checkout is confirmed, and on
/// downgrade when a subscription lapses.
pub async fn set_plan<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    plan: &str,
    subscription_status: Option<&str>,
    subscription_expires_at: Option<chrono::DateTime<chrono::Utc>>,
) -> SqlxResult<Option<ClubRow>> {
    sqlx::query_as::<_, ClubRow>(
        r#"
        UPDATE clubs
        SET plan = $2,
            subscription_status = $3,
            subscription_expires_at = $4
        WHERE id = $1
        RETURNING id, name, city, postal_code, province, country, address, vat_number, needs_review, plan, subscription_status, subscription_expires_at, created_at, updated_at
        "#,
    )
    .bind(club_id)
    .bind(plan)
    .bind(subscription_status)
    .bind(subscription_expires_at)
    .fetch_optional(executor)
    .await
}

/// Downgrade every paid club whose subscription has lapsed back to free, marking
/// it `expired`. Idempotent: once a club is on `free` it's no longer matched, and
/// clubs with no expiry (NULL — e.g. grandfathered) are never touched. Returns
/// the number of clubs downgraded.
pub async fn downgrade_expired<'e>(
    executor: impl PgExecutor<'e>,
    now: chrono::DateTime<chrono::Utc>,
) -> SqlxResult<u64> {
    let result = sqlx::query(
        r#"
        UPDATE clubs
        SET plan = 'free',
            subscription_status = 'expired'
        WHERE plan <> 'free'
          AND subscription_expires_at IS NOT NULL
          AND subscription_expires_at < $1
        "#,
    )
    .bind(now)
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}
