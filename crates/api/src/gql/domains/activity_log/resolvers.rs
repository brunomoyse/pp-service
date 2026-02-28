use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::gql::error::ResultExt;
use crate::gql::types::{
    ActivityEventCategory, ActivityLogEntry, PaginatedResponse, PaginationInput,
};
use crate::state::AppState;
use infra::repos::activity_log;

#[derive(Default)]
pub struct ActivityLogQuery;

#[Object]
impl ActivityLogQuery {
    /// Get activity log entries for a tournament, with optional category filter and pagination
    async fn tournament_activity_log(
        &self,
        ctx: &Context<'_>,
        tournament_id: Uuid,
        category: Option<ActivityEventCategory>,
        pagination: Option<PaginationInput>,
    ) -> Result<PaginatedResponse<ActivityLogEntry>> {
        let state = ctx.data::<AppState>()?;

        let page_params = pagination.unwrap_or(PaginationInput {
            limit: Some(50),
            offset: Some(0),
        });
        let limit_offset = page_params.to_limit_offset();

        let category_str = category.map(|c| c.as_str());

        let (rows, total_count) = tokio::try_join!(
            activity_log::list_by_tournament(
                &state.db,
                tournament_id,
                category_str,
                limit_offset.limit,
                limit_offset.offset,
            ),
            activity_log::count_by_tournament(&state.db, tournament_id, category_str)
        )
        .gql_err("Database operation failed")?;

        let items: Vec<ActivityLogEntry> = rows.into_iter().map(ActivityLogEntry::from).collect();
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
