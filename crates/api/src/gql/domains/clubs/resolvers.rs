use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use super::service;
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
                is_assigned: assigned_table_ids.contains(&table_row.id),
                created_at: table_row.created_at,
                updated_at: table_row.updated_at,
            })
            .collect())
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
}
