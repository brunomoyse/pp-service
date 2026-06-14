use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::auth::permissions::{require_admin, require_club_manager};
use crate::gql::common::helpers::get_club_id_for_tournament;
use crate::gql::error::ResultExt;
use crate::gql::types::{PaginatedResponse, PaginationInput};
use crate::state::AppState;
use infra::repos::announcements;

use super::service;
use super::types::{Announcement, AnnouncementScope, CreateAnnouncementInput};

fn default_page() -> PaginationInput {
    PaginationInput {
        limit: Some(50),
        offset: Some(0),
    }
}

#[derive(Default)]
pub struct AnnouncementQuery;

#[Object]
impl AnnouncementQuery {
    /// The current user's announcement feed: every platform announcement, the
    /// announcements of clubs they are a roster member of, and announcements for
    /// tournaments they are registered in. Newest first.
    async fn my_announcements(
        &self,
        ctx: &Context<'_>,
        pagination: Option<PaginationInput>,
    ) -> Result<PaginatedResponse<Announcement>> {
        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let limit_offset = pagination.unwrap_or_else(default_page).to_limit_offset();

        let (rows, total_count) = tokio::try_join!(
            announcements::list_for_user(&state.db, user_id, limit_offset),
            announcements::count_for_user(&state.db, user_id),
        )?;

        let items: Vec<Announcement> = rows.into_iter().map(Announcement::from).collect();
        let page_size = items.len() as i32;
        let offset = limit_offset.offset as i32;
        let has_next_page = (offset + page_size) < total_count as i32;

        Ok(PaginatedResponse {
            items,
            total_count: total_count as i32,
            page_size,
            offset,
            has_next_page,
        })
    }

    /// A club's announcements (its tournament + club scoped rows) for the
    /// management view. Managers of the club only.
    async fn club_announcements(
        &self,
        ctx: &Context<'_>,
        club_id: ID,
        pagination: Option<PaginationInput>,
    ) -> Result<PaginatedResponse<Announcement>> {
        let club_uuid = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_uuid).await?;

        let state = ctx.data::<AppState>()?;
        let limit_offset = pagination.unwrap_or_else(default_page).to_limit_offset();

        let (rows, total_count) = tokio::try_join!(
            announcements::list_by_club(&state.db, club_uuid, limit_offset),
            announcements::count_by_club(&state.db, club_uuid),
        )?;

        let items: Vec<Announcement> = rows.into_iter().map(Announcement::from).collect();
        let page_size = items.len() as i32;
        let offset = limit_offset.offset as i32;
        let has_next_page = (offset + page_size) < total_count as i32;

        Ok(PaginatedResponse {
            items,
            total_count: total_count as i32,
            page_size,
            offset,
            has_next_page,
        })
    }
}

#[derive(Default)]
pub struct AnnouncementMutation;

#[Object]
impl AnnouncementMutation {
    /// Author and broadcast an announcement. Authorization follows the scope:
    /// `TOURNAMENT`/`CLUB` require managing the club (the club is derived from
    /// the tournament for `TOURNAMENT`); `PLATFORM` requires an admin.
    async fn create_announcement(
        &self,
        ctx: &Context<'_>,
        input: CreateAnnouncementInput,
    ) -> Result<Announcement> {
        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let created_by = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let (club_id, tournament_id) = match input.scope {
            AnnouncementScope::Tournament => {
                let tid = input.tournament_id.as_ref().ok_or_else(|| {
                    async_graphql::Error::new("tournamentId is required for the TOURNAMENT scope")
                })?;
                let tid = Uuid::parse_str(tid.as_str()).gql_err("Invalid tournament ID")?;
                let club_id = get_club_id_for_tournament(&state.db, tid).await?;
                require_club_manager(ctx, club_id).await?;
                (Some(club_id), Some(tid))
            }
            AnnouncementScope::Club => {
                let cid = input.club_id.as_ref().ok_or_else(|| {
                    async_graphql::Error::new("clubId is required for the CLUB scope")
                })?;
                let cid = Uuid::parse_str(cid.as_str()).gql_err("Invalid club ID")?;
                require_club_manager(ctx, cid).await?;
                (Some(cid), None)
            }
            AnnouncementScope::Platform => {
                require_admin(ctx).await?;
                (None, None)
            }
        };

        let row = service::create_announcement(
            &state.db,
            input.scope,
            club_id,
            tournament_id,
            &input.title,
            &input.body,
            created_by,
        )
        .await?;

        Ok(Announcement::from(row))
    }
}
