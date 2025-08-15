use async_graphql::{Context, Object, Result};
use chrono::{DateTime, Utc};

use crate::state::AppState;
use infra::{repos::{ClubRepo, TournamentRepo, TournamentFilter, UserRepo, UserFilter, TournamentRegistrationRepo, TournamentResultRepo, UserStatistics}, pagination::LimitOffset};

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
        status: Option<crate::gql::types::TournamentStatus>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> async_graphql::Result<Vec<crate::gql::types::Tournament>> {
        let state = ctx.data::<AppState>()?;
        let repo = TournamentRepo::new(state.db.clone());
        let filter = TournamentFilter { 
            club_id, 
            from, 
            to, 
            status: status.map(|s| s.into())
        };
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
            role: crate::gql::types::Role::from(r.role),
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
                    role: crate::gql::types::Role::from(user_row.role),
                };
                
                players.push(crate::gql::types::TournamentPlayer {
                    registration: tournament_registration,
                    user,
                });
            }
        }
        
        Ok(players)
    }

    async fn my_tournament_registrations(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<crate::gql::types::TournamentRegistration>> {
        use crate::auth::Claims;
        
        // Get authenticated user from JWT token
        let claims = ctx.data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;
        
        let user_id = uuid::Uuid::parse_str(&claims.sub)
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;

        let state = ctx.data::<AppState>()?;
        let registration_repo = TournamentRegistrationRepo::new(state.db.clone());
        
        let registrations = registration_repo.get_user_current_registrations(user_id).await?;
        
        Ok(registrations.into_iter().map(|r| crate::gql::types::TournamentRegistration {
            id: r.id.into(),
            tournament_id: r.tournament_id.into(),
            user_id: r.user_id.into(),
            registration_time: r.registration_time,
            status: r.status,
            notes: r.notes,
        }).collect())
    }

    async fn my_recent_tournament_results(
        &self,
        ctx: &Context<'_>,
        limit: Option<i64>,
    ) -> Result<Vec<crate::gql::types::UserTournamentResult>> {
        use crate::auth::Claims;
        
        // Get authenticated user from JWT token
        let claims = ctx.data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;
        
        let user_id = uuid::Uuid::parse_str(&claims.sub)
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;

        let state = ctx.data::<AppState>()?;
        let result_repo = TournamentResultRepo::new(state.db.clone());
        let tournament_repo = TournamentRepo::new(state.db.clone());
        
        let limit = limit.unwrap_or(10).clamp(1, 50);
        let results = result_repo.get_user_recent_results(user_id, limit).await?;
        
        let mut user_results = Vec::new();
        for result_row in results {
            if let Some(tournament_row) = tournament_repo.get(result_row.tournament_id).await? {
                let tournament_result = crate::gql::types::TournamentResult {
                    id: result_row.id.into(),
                    tournament_id: result_row.tournament_id.into(),
                    user_id: result_row.user_id.into(),
                    final_position: result_row.final_position,
                    prize_cents: result_row.prize_cents,
                    notes: result_row.notes,
                    created_at: result_row.created_at,
                };

                let tournament = crate::gql::types::Tournament {
                    id: tournament_row.id.into(),
                    title: tournament_row.name,
                    club_id: tournament_row.club_id.into(),
                };

                user_results.push(crate::gql::types::UserTournamentResult {
                    result: tournament_result,
                    tournament,
                });
            }
        }
        
        Ok(user_results)
    }

    async fn my_tournament_statistics(
        &self,
        ctx: &Context<'_>,
    ) -> Result<crate::gql::types::PlayerStatsResponse> {
        use crate::auth::Claims;
        
        // Get authenticated user from JWT token
        let claims = ctx.data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;
        
        let user_id = uuid::Uuid::parse_str(&claims.sub)
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;

        let state = ctx.data::<AppState>()?;
        let result_repo = TournamentResultRepo::new(state.db.clone());
        
        // Get statistics for different time periods
        let stats_7_days = result_repo.get_user_statistics(user_id, 7).await?;
        let stats_30_days = result_repo.get_user_statistics(user_id, 30).await?;
        let stats_year = result_repo.get_user_statistics(user_id, 365).await?;
        
        // Convert to GraphQL types
        let convert_stats = |stats: UserStatistics| crate::gql::types::PlayerStatistics {
            total_itm: stats.total_itm,
            total_tournaments: stats.total_tournaments,
            total_winnings: stats.total_winnings,
            total_buy_ins: stats.total_buy_ins,
            itm_percentage: stats.itm_percentage,
            roi_percentage: stats.roi_percentage,
        };
        
        Ok(crate::gql::types::PlayerStatsResponse {
            last_7_days: convert_stats(stats_7_days),
            last_30_days: convert_stats(stats_30_days),
            last_year: convert_stats(stats_year),
        })
    }
}