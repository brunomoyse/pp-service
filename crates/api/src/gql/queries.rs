use async_graphql::{Context, Object, Result};
use chrono::{DateTime, Utc};

use crate::state::AppState;
use super::types::{Tournament, Club};

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

    /// Example: list tournaments (stubbed). Shows how to access DB if needed.
    async fn tournaments(&self, ctx: &Context<'_>) -> Result<Vec<Tournament>> {
        let _state = ctx.data::<AppState>()?;

        // Minimal stub data. Replace with sqlx query later.
        Ok(vec![
            Tournament { id: "t1".into(), title: "Friday Night".into(), club_id: "c1".into() },
            Tournament { id: "t2".into(), title: "Sunday Deepstack".into(), club_id: "c1".into() },
        ])
    }

    /// Example: list clubs (stubbed).
    async fn clubs(&self, _ctx: &Context<'_>) -> Result<Vec<Club>> {
        Ok(vec![
            Club { id: "c1".into(), name: "Brussels Poker Club".into(), city: Some("Bruxelles".into()) },
        ])
    }
}