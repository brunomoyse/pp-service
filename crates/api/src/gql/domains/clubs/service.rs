//! Self-serve club onboarding.
//!
//! `onboard_club` atomically creates the owner's account, their club, and the
//! `club_managers` link in a single transaction, then mints a JWT so the client
//! logs straight in. The VAT/enterprise number is validated for format and
//! verified against VIES (anti-abuse gate) before any rows are written.

use async_graphql::Result;

use super::types::{Club, ClubPlan, OnboardClubInput, OnboardClubPayload};
use crate::auth::password::PasswordService;
use crate::gql::error::ResultExt;
use crate::gql::types::User;
use crate::services::vies;
use crate::state::AppState;
use infra::repos::clubs::CreateClubData;

/// Supported onboarding countries (ISO 2-letter codes).
const SUPPORTED_COUNTRIES: [&str; 4] = ["BE", "FR", "LU", "NL"];

/// Strip everything but alphanumerics, uppercase, and drop a leading country
/// code if the user typed it, yielding the bare national number.
fn normalize_vat(country: &str, vat: &str) -> String {
    let cleaned: String = vat
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_uppercase();
    cleaned
        .strip_prefix(&country.to_uppercase())
        .unwrap_or(&cleaned)
        .to_string()
}

/// Validate the bare national VAT number against the country's format.
/// Mirrors the client-side `utils/vat.ts` regexes.
fn vat_format_ok(country: &str, number: &str) -> bool {
    let digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    let alnum = |s: &str| s.chars().all(|c| c.is_ascii_alphanumeric());
    match country {
        // BE: 10 digits starting with 0 or 1
        "BE" => number.len() == 10 && digits(number) && matches!(&number[0..1], "0" | "1"),
        // FR: 2 alphanumeric key chars + 9-digit SIREN
        "FR" => number.len() == 11 && alnum(&number[0..2]) && digits(&number[2..]),
        // LU: 8 digits
        "LU" => number.len() == 8 && digits(number),
        // NL: 9 digits + 'B' + 2 digits
        "NL" => {
            number.len() == 12
                && digits(&number[0..9])
                && &number[9..10] == "B"
                && digits(&number[10..])
        }
        _ => false,
    }
}

/// Whether a VIES "not found" should reject onboarding. Defaults to `true`;
/// flip with `VIES_HARD_BLOCK=false` if false-rejects (non-VAT non-profits)
/// become a problem in practice.
fn vies_hard_block() -> bool {
    std::env::var("VIES_HARD_BLOCK")
        .map(|v| !matches!(v.trim().to_lowercase().as_str(), "false" | "0" | "no"))
        .unwrap_or(true)
}

pub async fn onboard_club(state: &AppState, input: OnboardClubInput) -> Result<OnboardClubPayload> {
    let country = input.country.trim().to_uppercase();
    if !SUPPORTED_COUNTRIES.contains(&country.as_str()) {
        return Err(async_graphql::Error::new("Unsupported country"));
    }

    // Free is the default self-serve tier. Casino is sales-led — not creatable
    // through public onboarding.
    let plan = input.plan.unwrap_or(ClubPlan::Free);
    if plan == ClubPlan::Casino {
        return Err(async_graphql::Error::new(
            "The Casino plan is set up with our team — please contact us",
        ));
    }

    // VAT/VIES gating only applies to the paid Club (business) path. The free
    // home-game path skips it entirely: no VAT stored, never flagged for review.
    let (stored_vat, needs_review) = if plan == ClubPlan::Club {
        // 1. VAT format (cheap, before any network or DB work)
        let raw_vat = input.vat_number.as_deref().unwrap_or("");
        let vat_number = normalize_vat(&country, raw_vat);
        if !vat_format_ok(&country, &vat_number) {
            return Err(async_graphql::Error::new(
                "Invalid VAT number for the selected country",
            ));
        }

        // 2. VIES registry check. Unreachable VIES (available == false) never
        //    blocks; a definitive "not found" blocks only when the hard-block
        //    flag is on.
        let lookup = vies::lookup(&country, &vat_number).await;
        if lookup.available && !lookup.valid && vies_hard_block() {
            return Err(async_graphql::Error::new(
                "No company found for this VAT number",
            ));
        }

        // Non-profits (ASBL/VZW) verified via VIES are auto-approved; anything we
        // couldn't confirm as a non-profit (other legal form, or VIES
        // unreachable) is still created but flagged for manual review.
        let needs_review = !(lookup.available && lookup.valid && lookup.non_profit);
        (Some(format!("{country}{vat_number}")), needs_review)
    } else {
        (None, false)
    };

    // 3. Password strength
    PasswordService::validate_password_strength(&input.password)
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;
    let password_hash =
        PasswordService::hash_password(&input.password).gql_err("Failed to process password")?;

    // 4. Email must be free
    let existing = sqlx::query("SELECT id FROM users WHERE email = $1")
        .bind(&input.email)
        .fetch_optional(&state.db)
        .await
        .gql_err("Database operation failed")?;
    if existing.is_some() {
        return Err(async_graphql::Error::new(
            "A user with this email already exists",
        ));
    }

    // 5. Transaction: user → club → club_manager link (all-or-nothing)
    let mut tx = state
        .db
        .begin()
        .await
        .gql_err("Database operation failed")?;

    let user_row = sqlx::query_as::<_, infra::models::UserRow>(
        r#"
        INSERT INTO users (email, first_name, last_name, password_hash, role, is_active)
        VALUES ($1, $2, $3, $4, 'manager', true)
        RETURNING id, email, username, first_name, last_name, phone, is_active, role, locale, created_at, updated_at
        "#,
    )
    .bind(&input.email)
    .bind(&input.first_name)
    .bind(&input.last_name)
    .bind(&password_hash)
    .fetch_one(&mut *tx)
    .await
    .gql_err("Database operation failed")?;

    let club_row = infra::repos::clubs::create(
        &mut *tx,
        CreateClubData {
            name: input.club_name,
            address: input.address,
            city: input.city,
            postal_code: input.postal_code,
            country,
            vat_number: stored_vat,
            needs_review,
            plan: plan.as_db().to_string(),
        },
    )
    .await
    .gql_err("Database operation failed")?;

    sqlx::query("INSERT INTO club_managers (club_id, user_id) VALUES ($1, $2)")
        .bind(club_row.id)
        .bind(user_row.id)
        .execute(&mut *tx)
        .await
        .gql_err("Database operation failed")?;

    tx.commit().await.gql_err("Database operation failed")?;

    // 6. Mint JWT so the client logs straight in
    let user: User = user_row.into();
    let role_str: String = user.role.into();
    let token = state
        .jwt_service()
        .create_token(
            uuid::Uuid::parse_str(user.id.as_str()).gql_err("Invalid user ID")?,
            user.email.clone(),
            role_str,
        )
        .gql_err("Failed to issue token")?;

    Ok(OnboardClubPayload {
        token,
        user,
        club: Club::from(club_row),
    })
}
