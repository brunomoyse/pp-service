use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::auth::permissions::require_club_manager;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::club_players;

use super::service;
use super::types::{
    ArchiveClubPlayerInput, BulkRosterResult, ClaimClubPlayerInput, ClubPlayer,
    CreateClubPlayerInput, CreateClubPlayersBulkInput, FormatRosterImportInput, ImportCandidate,
    SkippedRow, UpdateClubPlayerInput,
};

#[derive(Default)]
pub struct IdentityQuery;

#[Object]
impl IdentityQuery {
    /// Full club roster (app users and non-users alike). Managers of the club only.
    async fn club_players(&self, ctx: &Context<'_>, club_id: ID) -> Result<Vec<ClubPlayer>> {
        let club_uuid = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_uuid).await?;

        let state = ctx.data::<AppState>()?;
        let rows = club_players::list_by_club(&state.db, club_uuid).await?;
        Ok(rows.into_iter().map(ClubPlayer::from).collect())
    }

    /// The current user's roster entries across every club — the cross-club profile.
    async fn my_cross_club_profile(&self, ctx: &Context<'_>) -> Result<Vec<ClubPlayer>> {
        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let rows = club_players::list_for_app_user(&state.db, user_id).await?;
        Ok(rows.into_iter().map(ClubPlayer::from).collect())
    }
}

#[derive(Default)]
pub struct IdentityMutation;

#[Object]
impl IdentityMutation {
    /// Add someone the club registers who is not (yet) an app user to the roster.
    /// Managers of the club only.
    async fn create_club_player(
        &self,
        ctx: &Context<'_>,
        input: CreateClubPlayerInput,
    ) -> Result<ClubPlayer> {
        let club_uuid = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_uuid).await?;

        let state = ctx.data::<AppState>()?;
        let row = service::create_roster_entry(&state.db, club_uuid, &input.display_name).await?;
        Ok(ClubPlayer::from(row))
    }

    /// Claim an unclaimed roster entry, linking it to the current app user.
    async fn claim_club_player(
        &self,
        ctx: &Context<'_>,
        input: ClaimClubPlayerInput,
    ) -> Result<ClubPlayer> {
        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;
        let rp_id =
            Uuid::parse_str(input.club_player_id.as_str()).gql_err("Invalid club player ID")?;

        let row = service::claim_roster_entry(&state.db, rp_id, user_id).await?;
        Ok(ClubPlayer::from(row))
    }

    /// Rename a roster entry. Managers of the entry's club only.
    async fn update_club_player(
        &self,
        ctx: &Context<'_>,
        input: UpdateClubPlayerInput,
    ) -> Result<ClubPlayer> {
        let state = ctx.data::<AppState>()?;
        let rp_id = Uuid::parse_str(input.id.as_str()).gql_err("Invalid club player ID")?;
        let club_id = roster_entry_club(state, rp_id).await?;
        require_club_manager(ctx, club_id).await?;

        let row =
            service::rename_roster_entry(&state.db, rp_id, club_id, &input.display_name).await?;
        Ok(ClubPlayer::from(row))
    }

    /// Archive (soft-delete) or restore a roster entry. Managers of the entry's
    /// club only. Archived entries are hidden from the roster but keep their
    /// historical registrations and results.
    async fn archive_club_player(
        &self,
        ctx: &Context<'_>,
        input: ArchiveClubPlayerInput,
    ) -> Result<ClubPlayer> {
        let state = ctx.data::<AppState>()?;
        let rp_id = Uuid::parse_str(input.id.as_str()).gql_err("Invalid club player ID")?;
        let club_id = roster_entry_club(state, rp_id).await?;
        require_club_manager(ctx, club_id).await?;

        let row =
            service::set_roster_entry_active(&state.db, rp_id, club_id, input.is_active).await?;
        Ok(ClubPlayer::from(row))
    }

    /// Normalize a parsed spreadsheet into clean roster display names via the
    /// configured AI model. Managers of the club only. Returns one candidate per
    /// usable input row; errors if AI import is not configured so the client can
    /// fall back to manual column mapping.
    async fn format_roster_import(
        &self,
        ctx: &Context<'_>,
        input: FormatRosterImportInput,
    ) -> Result<Vec<ImportCandidate>> {
        let club_uuid = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_uuid).await?;

        let state = ctx.data::<AppState>()?;
        let openrouter = state
            .openrouter_service()
            .ok_or_else(|| async_graphql::Error::new("AI import not configured"))?;

        let normalized = openrouter
            .normalize_roster(&input.headers, &input.rows)
            .await
            .map_err(|e| async_graphql::Error::new(format!("AI formatting failed: {e}")))?;

        Ok(normalized
            .into_iter()
            .map(|p| ImportCandidate {
                source_row_index: p.source_row_index as i32,
                display_name: p.display_name,
            })
            .collect())
    }

    /// Bulk-create roster entries from confirmed display names. Managers of the
    /// club only. De-duplicates against the existing roster and within the batch,
    /// reporting skipped rows.
    async fn create_club_players_bulk(
        &self,
        ctx: &Context<'_>,
        input: CreateClubPlayersBulkInput,
    ) -> Result<BulkRosterResult> {
        let club_uuid = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_uuid).await?;

        let state = ctx.data::<AppState>()?;
        let (created, skipped) =
            service::create_roster_entries_bulk(&state.db, club_uuid, &input.display_names).await?;

        Ok(BulkRosterResult {
            created: created.into_iter().map(ClubPlayer::from).collect(),
            skipped: skipped
                .into_iter()
                .map(|s| SkippedRow {
                    display_name: s.display_name,
                    reason: s.reason,
                })
                .collect(),
        })
    }
}

/// Resolve the club a roster entry belongs to (for club-scoped authorization).
async fn roster_entry_club(state: &AppState, rp_id: Uuid) -> Result<Uuid> {
    let entry = club_players::get_by_id(&state.db, rp_id)
        .await?
        .ok_or_else(|| async_graphql::Error::new("Roster entry not found"))?;
    Ok(entry.club_id)
}
