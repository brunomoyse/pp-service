use async_graphql::{Context, Object, Result};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::gql::error::ResultExt;
use crate::gql::types::{PaginatedResponse, PaginationInput, Tournament, TournamentStatus};
use crate::state::AppState;
use infra::repos::tournaments::{self, TournamentFilter};

#[derive(Default)]
pub struct TournamentQuery;

#[Object]
impl TournamentQuery {
    /// Get tournaments with optional filtering and pagination
    async fn tournaments(
        &self,
        ctx: &Context<'_>,
        club_id: Option<Uuid>,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
        status: Option<TournamentStatus>,
        pagination: Option<PaginationInput>,
    ) -> Result<PaginatedResponse<Tournament>> {
        let state = ctx.data::<AppState>()?;

        let filter = TournamentFilter {
            club_id,
            from,
            to,
            status: status.map(|s| s.into()),
        };

        let page_params = pagination.unwrap_or(PaginationInput {
            limit: Some(50),
            offset: Some(0),
        });
        let limit_offset = page_params.to_limit_offset();

        // Fetch tournaments and total count in parallel
        let (rows, total_count) = tokio::try_join!(
            tournaments::list(&state.db, filter.clone(), Some(limit_offset)),
            tournaments::count(&state.db, filter)
        )
        .gql_err("Database operation failed")?;

        let items: Vec<Tournament> = rows.into_iter().map(Tournament::from).collect();
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

    /// Get a single tournament by ID
    async fn tournament(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<Tournament>> {
        let state = ctx.data::<AppState>()?;

        let row = tournaments::get_by_id(&state.db, id)
            .await
            .gql_err("Database operation failed")?;

        Ok(row.map(Tournament::from))
    }
}
