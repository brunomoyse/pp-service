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
}

pub async fn list<'e>(executor: impl PgExecutor<'e>) -> SqlxResult<Vec<ClubRow>> {
    sqlx::query_as::<_, ClubRow>(
        r#"
        SELECT id, name, city, postal_code, province, country, address, vat_number, needs_review, created_at, updated_at
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
        WHERE province IS NOT NULL
        ORDER BY province ASC
        "#,
    )
    .fetch_all(executor)
    .await
}

pub async fn get_by_id<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> SqlxResult<Option<ClubRow>> {
    sqlx::query_as::<_, ClubRow>(
        r#"
        SELECT id, name, city, postal_code, province, country, address, vat_number, needs_review, created_at, updated_at
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
        INSERT INTO clubs (name, address, city, postal_code, country, vat_number, needs_review)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, name, city, postal_code, province, country, address, vat_number, needs_review, created_at, updated_at
        "#,
    )
    .bind(&data.name)
    .bind(&data.address)
    .bind(&data.city)
    .bind(&data.postal_code)
    .bind(&data.country)
    .bind(&data.vat_number)
    .bind(data.needs_review)
    .fetch_one(executor)
    .await
}
