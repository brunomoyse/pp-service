use async_graphql::{Enum, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};

use crate::gql::types::User;

/// Billing tier a club is on. Only `Free` is feature-gated (single table, one
/// active tournament, no recurring); `Club`/`Casino` are unlimited.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum ClubPlan {
    Free,
    Club,
    Casino,
}

impl ClubPlan {
    /// The value stored in `clubs.plan`.
    pub fn as_db(self) -> &'static str {
        match self {
            ClubPlan::Free => "free",
            ClubPlan::Club => "club",
            ClubPlan::Casino => "casino",
        }
    }

    /// Parse the stored `clubs.plan` value; unknown values fall back to `Free`.
    pub fn from_db(s: &str) -> Self {
        match s {
            "club" => ClubPlan::Club,
            "casino" => ClubPlan::Casino,
            _ => ClubPlan::Free,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct Club {
    pub id: ID,
    pub name: String,
    pub city: Option<String>,
    pub postal_code: Option<String>,
    /// Province slug derived from the postal code (e.g. "liege", "antwerp").
    /// Stable i18n key — localize client-side, don't display raw.
    pub province: Option<String>,
    pub address: Option<String>,
    pub vat_number: Option<String>,
    /// True when onboarding couldn't confirm the club as a non-profit; the club
    /// is active but awaiting manual review.
    pub needs_review: bool,
    /// Current billing tier; drives feature gating in the apps.
    pub plan: ClubPlan,
    /// When the current paid subscription lapses (null on free / no expiry).
    pub subscription_expires_at: Option<DateTime<Utc>>,
}

impl From<infra::models::ClubRow> for Club {
    fn from(row: infra::models::ClubRow) -> Self {
        Self {
            id: row.id.into(),
            name: row.name,
            city: row.city,
            postal_code: row.postal_code,
            province: row.province,
            address: row.address,
            vat_number: row.vat_number,
            needs_review: row.needs_review,
            plan: ClubPlan::from_db(&row.plan),
            subscription_expires_at: row.subscription_expires_at,
        }
    }
}

/// Self-serve onboarding payload: creates the owner's account **and** their
/// club in one transaction, returning a JWT so the client logs straight in.
#[derive(InputObject)]
pub struct OnboardClubInput {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub password: String,
    pub club_name: String,
    /// 2-letter ISO country code (BE/FR/LU/NL).
    pub country: String,
    /// Chosen tier. Omitted/`Free` takes the lightweight home-game path (no VAT);
    /// `Club` keeps the VAT + VIES business path. `Casino` is sales-led, not
    /// self-serve here.
    pub plan: Option<ClubPlan>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub postal_code: Option<String>,
    /// Required for the `Club` (paid) path; ignored on the free home-game path.
    pub vat_number: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct OnboardClubPayload {
    pub token: String,
    pub user: User,
    pub club: Club,
}

/// Result of a VIES company lookup, surfaced to the onboarding form so it can
/// confirm the company on blur and autofill the official name + split address.
#[derive(SimpleObject, Clone)]
pub struct CompanyLookup {
    /// Whether the VIES registry actually answered (false on outage/timeout).
    pub available: bool,
    /// Whether the VAT number resolves to a real registered company.
    pub valid: bool,
    /// Registered (legal) company name.
    pub name: Option<String>,
    /// Whether the company looks like a non-profit (ASBL/VZW/etc). When false,
    /// the club can still register but is flagged for manual review.
    pub non_profit: bool,
    pub street: Option<String>,
    pub postal_code: Option<String>,
    pub city: Option<String>,
}

impl From<crate::services::vies::CompanyLookupResult> for CompanyLookup {
    fn from(r: crate::services::vies::CompanyLookupResult) -> Self {
        Self {
            available: r.available,
            valid: r.valid,
            name: r.name,
            non_profit: r.non_profit,
            street: r.street,
            postal_code: r.postal_code,
            city: r.city,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct ClubTable {
    pub id: ID,
    pub club_id: ID,
    pub table_number: i32,
    pub max_seats: i32,
    pub is_active: bool,
    /// Whether this table is part of the club's default set, auto-linked to
    /// every newly created tournament.
    pub is_default: bool,
    pub is_assigned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Manager input to add a physical table to a club.
#[derive(InputObject)]
pub struct CreateClubTableInput {
    pub club_id: ID,
    pub table_number: i32,
    #[graphql(default = 9)]
    pub max_seats: i32,
    /// Whether the table joins the default set auto-linked to new tournaments.
    #[graphql(default = true)]
    pub is_default: bool,
}

/// Manager input to update a club table's seats / default membership / status.
#[derive(InputObject)]
pub struct UpdateClubTableInput {
    pub id: ID,
    pub max_seats: Option<i32>,
    pub is_default: Option<bool>,
    pub is_active: Option<bool>,
}

/// A redemption code that grants a club a fixed-length free trial on a paid
/// plan. Returned by the admin-only `createRedemptionCode` mutation.
#[derive(SimpleObject, Clone)]
pub struct RedemptionCode {
    pub id: ID,
    pub code: String,
    /// Tier this code upgrades a club to.
    pub plan: ClubPlan,
    /// Length of the free trial granted on redemption.
    pub trial_days: i32,
    /// Total redemptions allowed across all clubs; null = unlimited.
    pub max_uses: Option<i32>,
    pub used_count: i32,
    /// When the code stops being redeemable; null = never.
    pub expires_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<infra::models::RedemptionCodeRow> for RedemptionCode {
    fn from(row: infra::models::RedemptionCodeRow) -> Self {
        Self {
            id: row.id.into(),
            code: row.code,
            plan: ClubPlan::from_db(&row.plan),
            trial_days: row.trial_days,
            max_uses: row.max_uses,
            used_count: row.used_count,
            expires_at: row.expires_at,
            note: row.note,
            created_at: row.created_at,
        }
    }
}

/// Admin input to mint a redemption code. Omit `code` to auto-generate a unique
/// OTP-style code (`PP……`); any supplied code is normalized (separators
/// stripped, upper-cased) server-side.
#[derive(InputObject)]
pub struct CreateRedemptionCodeInput {
    pub code: Option<String>,
    /// Tier to grant. Defaults to `Club`; `Free` is rejected.
    pub plan: Option<ClubPlan>,
    #[graphql(default = 90)]
    pub trial_days: i32,
    /// Total redemptions allowed; defaults to 1 (single-use, OTP-style). Pass a
    /// higher number for a shared code, or 0 is rejected.
    #[graphql(default = 1)]
    pub max_uses: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
}
