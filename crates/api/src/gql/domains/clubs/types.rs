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
