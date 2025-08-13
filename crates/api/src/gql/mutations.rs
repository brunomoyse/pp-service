use async_graphql::{Context, InputObject, Object, Result, ID};

use crate::state::AppState;
use super::types::{Tournament, TournamentRegistration, RegisterForTournamentInput};
use infra::repos::{TournamentRegistrationRepo, CreateTournamentRegistration};
use uuid::Uuid;

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

    /// Register a user for a tournament.
    async fn register_for_tournament(
        &self,
        ctx: &Context<'_>,
        input: RegisterForTournamentInput,
    ) -> Result<TournamentRegistration> {
        let state = ctx.data::<AppState>()?;
        let repo = TournamentRegistrationRepo::new(state.db.clone());

        // TODO: Extract user_id from authentication token once auth is implemented
        let user_id = Uuid::parse_str("9fddc582-adb6-4d3e-a7c1-33a2d9608ad3").unwrap();
        
        let tournament_id = Uuid::parse_str(input.tournament_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament_id: {}", e)))?;

        let create_data = CreateTournamentRegistration {
            tournament_id,
            user_id,
            notes: input.notes,
        };

        let row = repo.create(create_data).await?;

        Ok(TournamentRegistration {
            id: row.id.into(),
            tournament_id: row.tournament_id.into(),
            user_id: row.user_id.into(),
            registration_time: row.registration_time,
            status: row.status,
            notes: row.notes,
        })
    }
}