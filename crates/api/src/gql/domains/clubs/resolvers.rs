use async_graphql::{Context, Object, Result};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::service;
use super::types::{
    ClubPlan, CreateClubTableInput, CreateRedemptionCodeInput, RedemptionCode, UpdateClubTableInput,
};
use crate::auth::permissions::{
    is_free_plan, require_admin, require_club_manager, viewer_is_admin,
};
use crate::gql::error::ResultExt;
use crate::gql::types::{Club, ClubTable, CompanyLookup, OnboardClubInput, OnboardClubPayload};
use crate::services::vies;
use crate::state::AppState;
use infra::repos::{club_tables, clubs, redemption_codes, table_seat_assignments, tournaments};

#[derive(Default)]
pub struct ClubQuery;

#[Object]
impl ClubQuery {
    async fn clubs(&self, ctx: &Context<'_>) -> Result<Vec<Club>> {
        let state = ctx.data::<AppState>()?;
        let rows = clubs::list(&state.db).await?;
        // Free ("Home Game") clubs are private — keep them out of the public
        // club directory the player app browses. Admins still see everything.
        let admin = viewer_is_admin(ctx);
        Ok(rows
            .into_iter()
            .filter(|c| admin || c.plan != "free")
            .map(Club::from)
            .collect())
    }

    /// Distinct province slugs clubs resolve to — for a province leaderboard
    /// filter. Slugs are i18n keys; localize client-side.
    async fn club_provinces(&self, ctx: &Context<'_>) -> Result<Vec<String>> {
        let state = ctx.data::<AppState>()?;
        Ok(clubs::list_provinces(&state.db).await?)
    }

    /// Verify a company by VAT number against the EU VIES registry. Used by the
    /// self-serve onboarding form to confirm the company on blur and autofill
    /// its name/address. `available: false` means VIES didn't answer (treat as
    /// "couldn't verify", not "invalid"). Unauthenticated; onboarding is public.
    async fn lookup_company(
        &self,
        _ctx: &Context<'_>,
        country: String,
        vat_number: String,
    ) -> Result<CompanyLookup> {
        Ok(vies::lookup(&country, &vat_number).await.into())
    }

    /// List recently minted redemption codes (admin overview). Admin-only.
    async fn redemption_codes(&self, ctx: &Context<'_>) -> Result<Vec<RedemptionCode>> {
        require_admin(ctx).await?;
        let state = ctx.data::<AppState>()?;
        let rows = redemption_codes::list_recent(&state.db, 100)
            .await
            .gql_err("Failed to load codes")?;
        Ok(rows.into_iter().map(RedemptionCode::from).collect())
    }

    /// Get all tables for a club
    async fn club_tables(&self, ctx: &Context<'_>, club_id: Uuid) -> Result<Vec<ClubTable>> {
        let state = ctx.data::<AppState>()?;

        let table_rows = club_tables::list_by_club(&state.db, club_id).await?;

        // Get active tournaments for this club to determine table assignments
        let active_tournaments = tournaments::list(
            &state.db,
            tournaments::TournamentFilter {
                club_id: Some(club_id),
                from: None,
                to: None,
                status: None,
                // Internal table-assignment lookup for a club the caller is
                // already viewing; no player-facing free filter needed.
                exclude_free_clubs: false,
            },
            None,
        )
        .await?;

        // Collect assigned table IDs from active tournaments
        let mut assigned_table_ids = std::collections::HashSet::new();

        for tournament in active_tournaments {
            // Skip finished tournaments
            if matches!(
                tournament.live_status,
                tournaments::TournamentLiveStatus::Finished
            ) {
                continue;
            }

            // Get seat assignments for this tournament
            let assignments =
                table_seat_assignments::list_current_for_tournament(&state.db, tournament.id)
                    .await?;

            for assignment in assignments {
                assigned_table_ids.insert(assignment.club_table_id);
            }
        }

        Ok(table_rows
            .into_iter()
            .map(|table_row| ClubTable {
                id: table_row.id.into(),
                club_id: table_row.club_id.into(),
                table_number: table_row.table_number,
                max_seats: table_row.max_seats,
                is_active: table_row.is_active,
                is_default: table_row.is_default,
                is_assigned: assigned_table_ids.contains(&table_row.id),
                created_at: table_row.created_at,
                updated_at: table_row.updated_at,
            })
            .collect())
    }
}

/// Map a freshly fetched/created `ClubTableRow` to the GraphQL type. New or
/// just-mutated tables are not yet assigned to a tournament, so `is_assigned`
/// is false; the `club_tables` query computes the live value.
fn club_table_from_row(row: infra::models::ClubTableRow) -> ClubTable {
    ClubTable {
        id: row.id.into(),
        club_id: row.club_id.into(),
        table_number: row.table_number,
        max_seats: row.max_seats,
        is_active: row.is_active,
        is_default: row.is_default,
        is_assigned: false,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

#[derive(Default)]
pub struct ClubMutation;

#[Object]
impl ClubMutation {
    /// Self-serve onboarding: create the owner's account + their club in one
    /// transaction and return a JWT so the client logs straight in.
    /// Unauthenticated; this is the public signup entry point.
    async fn onboard_club(
        &self,
        ctx: &Context<'_>,
        input: OnboardClubInput,
    ) -> Result<OnboardClubPayload> {
        let state = ctx.data::<AppState>()?;
        service::onboard_club(state, input).await
    }

    /// Predefine a physical table for a club. Managers of the club only.
    async fn create_club_table(
        &self,
        ctx: &Context<'_>,
        input: CreateClubTableInput,
    ) -> Result<ClubTable> {
        let state = ctx.data::<AppState>()?;
        let club_id = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_id).await?;

        // Free ("Home Game") tier is single-table. The moment a second table is
        // needed, the club has outgrown free — point them at the upgrade.
        if is_free_plan(ctx, club_id).await? {
            let existing = club_tables::list_by_club(&state.db, club_id).await?;
            if !existing.is_empty() {
                return Err(async_graphql::Error::new(
                    "The Home Game (free) plan is limited to 1 table. Upgrade to Club to add more.",
                ));
            }
        }

        if input.table_number < 1 {
            return Err(async_graphql::Error::new("Table number must be positive"));
        }
        if input.max_seats < 2 {
            return Err(async_graphql::Error::new("A table needs at least 2 seats"));
        }

        let row = club_tables::create(
            &state.db,
            club_id,
            input.table_number,
            input.max_seats,
            input.is_default,
        )
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(db) = &e {
                if db.is_unique_violation() {
                    return async_graphql::Error::new(format!(
                        "Table {} already exists for this club",
                        input.table_number
                    ));
                }
            }
            async_graphql::Error::new("Failed to create table")
        })?;

        Ok(club_table_from_row(row))
    }

    /// Update a club table (seats, default-set membership, active flag).
    /// Managers of the table's club only.
    async fn update_club_table(
        &self,
        ctx: &Context<'_>,
        input: UpdateClubTableInput,
    ) -> Result<ClubTable> {
        let state = ctx.data::<AppState>()?;
        let table_id = Uuid::parse_str(input.id.as_str()).gql_err("Invalid table ID")?;

        let existing = club_tables::get_by_id(&state.db, table_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Table not found"))?;
        require_club_manager(ctx, existing.club_id).await?;

        if let Some(seats) = input.max_seats {
            if seats < 2 {
                return Err(async_graphql::Error::new("A table needs at least 2 seats"));
            }
        }

        let row = club_tables::update(
            &state.db,
            table_id,
            club_tables::UpdateClubTable {
                max_seats: input.max_seats,
                is_active: input.is_active,
                is_default: input.is_default,
            },
        )
        .await?
        .ok_or_else(|| async_graphql::Error::new("Table not found"))?;

        Ok(club_table_from_row(row))
    }

    /// Delete a club table. Managers of the table's club only. Refuses while the
    /// table is booked by a live (non-finished) tournament.
    async fn delete_club_table(&self, ctx: &Context<'_>, id: async_graphql::ID) -> Result<bool> {
        let state = ctx.data::<AppState>()?;
        let table_id = Uuid::parse_str(id.as_str()).gql_err("Invalid table ID")?;

        let existing = club_tables::get_by_id(&state.db, table_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Table not found"))?;
        require_club_manager(ctx, existing.club_id).await?;

        let conflicts =
            club_tables::active_table_conflicts(&state.db, &[table_id], Uuid::nil()).await?;
        if let Some(conflict) = conflicts.first() {
            return Err(async_graphql::Error::new(format!(
                "Table {} is in use by an active tournament ({}) and cannot be deleted",
                conflict.table_number, conflict.tournament_name
            )));
        }

        club_tables::delete(&state.db, table_id)
            .await
            .gql_err("Failed to delete table")
    }

    /// Set a club's billing plan + subscription lifecycle. Admin-only: this is
    /// the service-to-service entry point the payments microservice calls (with
    /// a service/admin token) after a confirmed Mollie checkout, and on
    /// downgrade when a subscription lapses.
    async fn set_club_plan(
        &self,
        ctx: &Context<'_>,
        club_id: async_graphql::ID,
        plan: ClubPlan,
        subscription_status: Option<String>,
        subscription_expires_at: Option<DateTime<Utc>>,
    ) -> Result<Club> {
        require_admin(ctx).await?;
        let state = ctx.data::<AppState>()?;
        let club_id = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;

        let row = clubs::set_plan(
            &state.db,
            club_id,
            plan.as_db(),
            subscription_status.as_deref(),
            subscription_expires_at,
        )
        .await
        .gql_err("Failed to update club plan")?
        .ok_or_else(|| async_graphql::Error::new("Club not found"))?;

        Ok(Club::from(row))
    }

    /// Redeem a code to put the manager's club onto a paid plan for a free trial
    /// window (the manual / promo counterpart to a Mollie checkout). Managers of
    /// the club only. One redemption per club; the subscription-expiry sweep
    /// downgrades the club back to free when the trial lapses.
    async fn redeem_code(
        &self,
        ctx: &Context<'_>,
        club_id: async_graphql::ID,
        code: String,
    ) -> Result<Club> {
        let state = ctx.data::<AppState>()?;
        let club_id = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;
        let user = require_club_manager(ctx, club_id).await?;
        let user_id = Uuid::parse_str(user.id.as_str()).gql_err("Invalid user ID")?;

        let normalized = normalize_code(&code);
        if normalized.is_empty() {
            return Err(async_graphql::Error::new("Enter a code to redeem."));
        }

        // Hold a row lock on the code for the whole grant so concurrent
        // redemptions of a capped code can't oversell its last use.
        let mut tx = state
            .db
            .begin()
            .await
            .gql_err("Failed to start redemption")?;

        let code_row = redemption_codes::lock_by_code(&mut *tx, &normalized)
            .await
            .gql_err("Failed to look up code")?
            .ok_or_else(|| async_graphql::Error::new("That code isn't valid."))?;

        let now = Utc::now();
        if code_row.expires_at.is_some_and(|exp| exp < now) {
            return Err(async_graphql::Error::new("This code has expired."));
        }
        if code_row
            .max_uses
            .is_some_and(|max| code_row.used_count >= max)
        {
            return Err(async_graphql::Error::new(
                "This code has reached its redemption limit.",
            ));
        }
        if redemption_codes::has_used(&mut *tx, code_row.id, club_id)
            .await
            .gql_err("Failed to check redemption")?
        {
            return Err(async_graphql::Error::new(
                "This club has already redeemed this code.",
            ));
        }

        let expires_at = now + chrono::Duration::days(code_row.trial_days as i64);
        let club_row = clubs::set_plan(
            &mut *tx,
            club_id,
            &code_row.plan,
            Some("trial"),
            Some(expires_at),
        )
        .await
        .gql_err("Failed to apply plan")?
        .ok_or_else(|| async_graphql::Error::new("Club not found"))?;

        redemption_codes::insert_use(&mut *tx, code_row.id, club_id, user_id)
            .await
            .gql_err("Failed to record redemption")?;
        redemption_codes::increment_used(&mut *tx, code_row.id)
            .await
            .gql_err("Failed to record redemption")?;

        tx.commit().await.gql_err("Failed to finalize redemption")?;

        Ok(Club::from(club_row))
    }

    /// Mint a redemption code. Admin-only — used to hand out free-trial codes to
    /// pilot clubs.
    async fn create_redemption_code(
        &self,
        ctx: &Context<'_>,
        input: CreateRedemptionCodeInput,
    ) -> Result<RedemptionCode> {
        require_admin(ctx).await?;
        let state = ctx.data::<AppState>()?;

        if input.trial_days < 1 {
            return Err(async_graphql::Error::new("Trial length must be positive"));
        }
        if input.max_uses < 1 {
            return Err(async_graphql::Error::new("Max uses must be at least 1"));
        }
        let plan = input.plan.unwrap_or(ClubPlan::Club);
        if plan == ClubPlan::Free {
            return Err(async_graphql::Error::new(
                "A code can only grant a paid plan",
            ));
        }

        // An explicit code is used verbatim; otherwise generate a unique
        // OTP-style one, retrying on the unlikely collision.
        let explicit = input
            .code
            .as_deref()
            .map(normalize_code)
            .filter(|c| !c.is_empty());

        let mut attempts = 0;
        let row = loop {
            let candidate = match &explicit {
                Some(c) => c.clone(),
                None => redemption_codes::generate_code(),
            };
            match redemption_codes::create(
                &state.db,
                &candidate,
                plan.as_db(),
                input.trial_days,
                Some(input.max_uses),
                input.expires_at,
                input.note.as_deref(),
            )
            .await
            {
                Ok(row) => break row,
                Err(e) => {
                    let is_dup =
                        matches!(&e, sqlx::Error::Database(db) if db.is_unique_violation());
                    if is_dup && explicit.is_some() {
                        return Err(async_graphql::Error::new(
                            "A code with that value already exists",
                        ));
                    }
                    if is_dup && attempts < 5 {
                        attempts += 1;
                        continue;
                    }
                    return Err(async_graphql::Error::new("Failed to create code"));
                }
            }
        };

        Ok(RedemptionCode::from(row))
    }
}

/// Normalize a redemption code for storage and lookup: keep only letters and
/// digits (so `PP-XXXX-XXXX`, `pp xxxx xxxx`, etc. all resolve to the same
/// value) and upper-case it.
fn normalize_code(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_uppercase())
        .collect()
}
