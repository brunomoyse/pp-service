use async_graphql::{Context, Object, Result};
use chrono::{DateTime, Utc};

use crate::state::AppState;
use infra::{repos::{ClubRepo, TournamentRepo, TournamentFilter, UserRepo, UserFilter, TournamentRegistrationRepo}, pagination::LimitOffset};

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

    async fn users(
        &self,
        ctx: &Context<'_>,
        search: Option<String>,
        is_active: Option<bool>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<crate::gql::types::User>> {
        let state = ctx.data::<AppState>()?;
        let repo = UserRepo::new(state.db.clone());
        let filter = UserFilter { search, is_active };
        let page = Some(LimitOffset {
            limit: limit.unwrap_or(50).clamp(1, 200),
            offset: offset.unwrap_or(0).max(0),
        });
        let rows = repo.list(filter, page).await?;
        Ok(rows.into_iter().map(|r| crate::gql::types::User {
            id: r.id.into(),
            email: r.email,
            username: r.username,
            first_name: r.first_name,
            last_name: r.last_name,
            phone: r.phone,
            is_active: r.is_active,
            role: r.role.unwrap_or_else(|| "user".to_string()),
        }).collect())
    }

    async fn tournament_players(
        &self,
        ctx: &Context<'_>,
        tournament_id: uuid::Uuid,
    ) -> Result<Vec<crate::gql::types::TournamentPlayer>> {
        let state = ctx.data::<AppState>()?;
        let registration_repo = TournamentRegistrationRepo::new(state.db.clone());
        let user_repo = UserRepo::new(state.db.clone());
        
        let registrations = registration_repo.get_by_tournament(tournament_id).await?;
        
        let mut players = Vec::new();
        for registration in registrations {
            if let Some(user_row) = user_repo.get_by_id(registration.user_id).await? {
                let tournament_registration = crate::gql::types::TournamentRegistration {
                    id: registration.id.into(),
                    tournament_id: registration.tournament_id.into(),
                    user_id: registration.user_id.into(),
                    registration_time: registration.registration_time,
                    status: registration.status,
                    notes: registration.notes,
                };
                
                let user = crate::gql::types::User {
                    id: user_row.id.into(),
                    email: user_row.email,
                    username: user_row.username,
                    first_name: user_row.first_name,
                    last_name: user_row.last_name,
                    phone: user_row.phone,
                    is_active: user_row.is_active,
                    role: user_row.role.unwrap_or_else(|| "user".to_string()),
                };
                
                players.push(crate::gql::types::TournamentPlayer {
                    registration: tournament_registration,
                    user,
                });
            }
        }
        
        Ok(players)
    }
}