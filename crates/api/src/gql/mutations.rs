use async_graphql::{Context, InputObject, Object, Result, ID};

use crate::state::AppState;
use super::types::Tournament;

pub struct MutationRoot;

#[derive(InputObject)]
pub struct CreateTournamentInput {
    pub title: String,
    pub club_id: ID,
}

#[Object]
impl MutationRoot {
    /// Minimal example mutation creating a tournament (stub).
    /// Replace with an INSERT via sqlx later.
    async fn create_tournament(
        &self,
        ctx: &Context<'_>,
        input: CreateTournamentInput,
    ) -> Result<Tournament> {
        let _state = ctx.data::<AppState>()?;
        // Example: persist with sqlx here using _state.db

        Ok(Tournament {
            id: "new_tournament_id".into(),
            title: input.title,
            club_id: input.club_id,
        })
    }
}