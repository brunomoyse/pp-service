use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::{tournament_entries, tournament_entries::CreateTournamentEntry, tournaments};

use super::types::{AddTournamentEntryInput, EntryType, TournamentEntry, TournamentEntryStats};

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
            total_rake_cents: stats.total_rake_cents as i32,
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
        let is_initial = matches!(input.entry_type, EntryType::Initial);

        let create_data = CreateTournamentEntry {
            tournament_id,
            user_id: Some(user_id),
            club_player_id: None,
            entry_type: String::from(input.entry_type),
            amount_cents,
            chips_received: input.chips_received,
            recorded_by: Some(manager_id),
            notes: input.notes,
        };

        let entry_row = tournament_entries::create(&state.db, create_data).await?;

        // Mandatory drink voucher: bought together with the initial buy-in. It is
        // excluded from the prize pool (paper voucher IRL). Keyed to the same roster
        // player as the buy-in; skipped if the tournament has no voucher.
        if is_initial && tournament.voucher_value_cents > 0 {
            let voucher_data = CreateTournamentEntry {
                tournament_id,
                user_id: entry_row.user_id,
                club_player_id: Some(entry_row.club_player_id),
                entry_type: "voucher".to_string(),
                amount_cents: tournament.voucher_value_cents,
                chips_received: None,
                recorded_by: Some(manager_id),
                notes: Some("Mandatory drink voucher".to_string()),
            };
            tournament_entries::create(&state.db, voucher_data).await?;
        }

        // Log activity
        {
            let db = state.db.clone();
            let entry_type_str = String::from(input.entry_type);
            tokio::spawn(async move {
                crate::gql::domains::activity_log::log_and_publish(
                    &db,
                    tournament_id,
                    "entry",
                    "added",
                    Some(manager_id),
                    Some(user_id),
                    serde_json::json!({"entry_type": entry_type_str, "amount_cents": amount_cents}),
                )
                .await;
            });
        }

        Ok(entry_row.into())
    }

    /// Grant the level-2 early-bird bonus to the given roster players (managers only).
    /// For each player still seated and not yet awarded, adds a chip-only bonus entry
    /// and flips the award flag. Idempotent. Returns the number of players awarded.
    async fn grant_level_two_bonus(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
        club_player_ids: Vec<ID>,
    ) -> Result<i32> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;
        let tournament_id =
            Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let tournament = tournaments::get_by_id(&state.db, tournament_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;
        require_club_manager(ctx, tournament.club_id).await?;

        let bonus_chips = tournament.level_two_bonus_chips.ok_or_else(|| {
            async_graphql::Error::new("This tournament has no level-2 bonus configured")
        })?;

        let rp_ids: Vec<Uuid> = club_player_ids
            .iter()
            .map(|id| Uuid::parse_str(id.as_str()).gql_err("Invalid club player ID"))
            .collect::<Result<Vec<_>>>()?;

        let awarded = tournament_entries::grant_level_two_bonus(
            &state.db,
            tournament_id,
            &rp_ids,
            bonus_chips,
        )
        .await
        .gql_err("Failed to grant level-2 bonus")?;

        Ok(awarded as i32)
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
        let manager = require_club_manager(ctx, club_id).await?;
        let manager_id = Uuid::parse_str(manager.id.as_str()).ok();

        let result = tournament_entries::delete(&state.db, entry_id)
            .await
            .gql_err("Failed to delete entry")?;

        // Log activity
        {
            let db = state.db.clone();
            let t_id = entry.tournament_id;
            let u_id = entry.user_id;
            let entry_type = entry.entry_type.clone();
            let amount = entry.amount_cents;
            tokio::spawn(async move {
                crate::gql::domains::activity_log::log_and_publish(
                    &db,
                    t_id,
                    "entry",
                    "deleted",
                    manager_id,
                    u_id,
                    serde_json::json!({"entry_type": entry_type, "amount_cents": amount}),
                )
                .await;
            });
        }

        Ok(result)
    }
}
