use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::{tournament_entries, tournament_entries::CreateTournamentEntry, tournaments};

use super::types::{AddTournamentEntryInput, TournamentEntry, TournamentEntryStats};

#[derive(Default)]
pub struct EntryQuery;

#[Object]
impl EntryQuery {
    /// Get all entries for a tournament
    async fn tournament_entries(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<Vec<TournamentEntry>> {
        let state = ctx.data::<AppState>()?;
        let tournament_id =
            Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let entries = tournament_entries::list_by_tournament(&state.db, tournament_id).await?;

        Ok(entries.into_iter().map(TournamentEntry::from).collect())
    }

    /// Get entry statistics for a tournament
    async fn tournament_entry_stats(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<TournamentEntryStats> {
        let state = ctx.data::<AppState>()?;
        let tournament_id =
            Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let stats = tournament_entries::get_stats(&state.db, tournament_id).await?;

        Ok(TournamentEntryStats {
            tournament_id: tournament_id.into(),
            total_entries: stats.total_entries as i32,
            total_amount_cents: stats.total_amount_cents as i32,
            unique_players: stats.unique_players as i32,
            initial_count: stats.initial_count as i32,
            rebuy_count: stats.rebuy_count as i32,
            re_entry_count: stats.re_entry_count as i32,
            addon_count: stats.addon_count as i32,
        })
    }
}

#[derive(Default)]
pub struct EntryMutation;

#[Object]
impl EntryMutation {
    /// Add a tournament entry (initial buy-in, rebuy, re-entry, or add-on)
    /// Requires club manager permission for the tournament's club
    async fn add_tournament_entry(
        &self,
        ctx: &Context<'_>,
        input: AddTournamentEntryInput,
    ) -> Result<TournamentEntry> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;
        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;
        let user_id = Uuid::parse_str(input.user_id.as_str()).gql_err("Invalid user ID")?;

        // Get club ID for the tournament to verify permissions
        let tournament = tournaments::get_by_id(&state.db, tournament_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;
        let club_id = tournament.club_id;

        // Require manager role for this specific club
        let manager = require_club_manager(ctx, club_id).await?;
        let manager_id = Uuid::parse_str(manager.id.as_str()).gql_err("Invalid manager ID")?;

        // Use provided amount or default to tournament buy_in_cents
        let amount_cents = input.amount_cents.unwrap_or(tournament.buy_in_cents);

        let create_data = CreateTournamentEntry {
            tournament_id,
            user_id,
            entry_type: String::from(input.entry_type),
            amount_cents,
            chips_received: input.chips_received,
            recorded_by: Some(manager_id),
            notes: input.notes,
        };

        let entry_row = tournament_entries::create(&state.db, create_data).await?;

        Ok(entry_row.into())
    }

    /// Delete a tournament entry (for corrections)
    /// Requires club manager permission for the tournament's club
    async fn delete_tournament_entry(&self, ctx: &Context<'_>, entry_id: ID) -> Result<bool> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;
        let entry_id = Uuid::parse_str(entry_id.as_str()).gql_err("Invalid entry ID")?;

        // Get entry to find tournament_id for permission check
        let entry = tournament_entries::get_by_id(&state.db, entry_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Entry not found"))?;

        // Get club ID for permission check
        let tournament = tournaments::get_by_id(&state.db, entry.tournament_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;
        let club_id = tournament.club_id;

        // Require manager role for this specific club
        let _manager = require_club_manager(ctx, club_id).await?;

        tournament_entries::delete(&state.db, entry_id)
            .await
            .gql_err("Failed to delete entry")
    }
}
