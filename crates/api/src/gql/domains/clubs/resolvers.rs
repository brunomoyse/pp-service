use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use super::service;
use super::types::{CreateClubTableInput, UpdateClubTableInput};
use crate::auth::permissions::require_club_manager;
use crate::gql::error::ResultExt;
use crate::gql::types::{Club, ClubTable, CompanyLookup, OnboardClubInput, OnboardClubPayload};
use crate::services::vies;
use crate::state::AppState;
use infra::repos::{club_tables, clubs, table_seat_assignments, tournaments};

#[derive(Default)]
pub struct ClubQuery;

#[Object]
impl ClubQuery {
    async fn clubs(&self, ctx: &Context<'_>) -> Result<Vec<Club>> {
        let state = ctx.data::<AppState>()?;
        let rows = clubs::list(&state.db).await?;
        Ok(rows.into_iter().map(Club::from).collect())
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
}
