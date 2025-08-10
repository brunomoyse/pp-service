use async_graphql::{Context, Object, Result};
use chrono::{DateTime, Utc};

use crate::state::AppState;
use infra::{repos::{ClubRepo, TournamentRepo, TournamentFilter}, pagination::LimitOffset};

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Simple ping to test the API.
    async fn hello(&self) -> String {
        "Hello, PocketPair!".to_string()
    }

    /// Current server time (UTC), example of returning chrono types.
    async fn server_time(&self) -> DateTime<Utc> {
        Utc::now()
    }

    async fn clubs(&self, ctx: &Context<'_>) -> Result<Vec<crate::gql::types::Club>> {
        let state = ctx.data::<AppState>()?;
        let repo = ClubRepo::new(state.db.clone());
        let rows = repo.list_all().await?;
        Ok(rows.into_iter().map(|r| crate::gql::types::Club {
            id: r.id.into(),
            name: r.name,
            city: r.city,
        }).collect())
    }

    async fn tournaments(
        &self,
        ctx: &async_graphql::Context<'_>,
        club_id: Option<uuid::Uuid>,
        from: Option<chrono::DateTime<chrono::Utc>>,
        to: Option<chrono::DateTime<chrono::Utc>>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> async_graphql::Result<Vec<crate::gql::types::Tournament>> {
        let state = ctx.data::<AppState>()?;
        let repo = TournamentRepo::new(state.db.clone());
        let filter = TournamentFilter { club_id, from, to };
        let page = Some(LimitOffset {
            limit: limit.unwrap_or(50).clamp(1, 200),
            offset: offset.unwrap_or(0).max(0),
        });
        let rows = repo.list(filter, page).await?;
        Ok(rows.into_iter().map(|r| crate::gql::types::Tournament {
            id: r.id.into(),
            title: r.name,
            club_id: r.club_id.into(),
        }).collect())
    }
}