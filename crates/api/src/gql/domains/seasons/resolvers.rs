use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::auth::permissions::require_club_manager;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::seasons as seasons_repo;

use super::service;
use super::types::{CreateSeasonInput, HallOfFameEntry, QuestProgress, Season, SeasonPass};

fn current_user_id(ctx: &Context<'_>) -> Result<Uuid> {
    let claims = ctx.data::<Claims>()?;
    Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")
}

#[derive(Default)]
pub struct SeasonsQuery;

#[Object]
impl SeasonsQuery {
    /// The club's currently-running season, if any.
    async fn current_season(&self, ctx: &Context<'_>, club_id: ID) -> Result<Option<Season>> {
        let state = ctx.data::<AppState>()?;
        let club = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;
        let row = seasons_repo::current_for_club(&state.db, club, Utc::now()).await?;
        Ok(row.map(service::to_season))
    }

    /// All of a club's seasons, newest first.
    async fn club_seasons(&self, ctx: &Context<'_>, club_id: ID) -> Result<Vec<Season>> {
        let state = ctx.data::<AppState>()?;
        let club = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;
        let rows = seasons_repo::list_by_club(&state.db, club).await?;
        Ok(rows.into_iter().map(service::to_season).collect())
    }

    /// The current user's pass standing for a season.
    async fn my_season_pass(&self, ctx: &Context<'_>, season_id: ID) -> Result<SeasonPass> {
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let sid = Uuid::parse_str(season_id.as_str()).gql_err("Invalid season ID")?;

        let season = seasons_repo::get_by_id(&state.db, sid)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Season not found"))?;
        Ok(service::compute_pass(&state.db, &season, user_id).await?)
    }

    /// This week's three rotating quests with the current user's progress.
    async fn weekly_quests(&self, ctx: &Context<'_>) -> Result<Vec<QuestProgress>> {
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        Ok(service::weekly_quests(&state.db, user_id).await?)
    }

    /// A club's Hall of Fame — the champion of every finished season.
    async fn club_hall_of_fame(
        &self,
        ctx: &Context<'_>,
        club_id: ID,
    ) -> Result<Vec<HallOfFameEntry>> {
        let state = ctx.data::<AppState>()?;
        let club = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;
        Ok(service::hall_of_fame(&state.db, club).await?)
    }
}

#[derive(Default)]
pub struct SeasonsMutation;

#[Object]
impl SeasonsMutation {
    /// Open a new season for a club. Managers of that club only.
    async fn create_season(&self, ctx: &Context<'_>, input: CreateSeasonInput) -> Result<Season> {
        let club = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club).await?;

        if input.ends_at <= input.starts_at {
            return Err(async_graphql::Error::new(
                "Season end must be after its start",
            ));
        }

        let state = ctx.data::<AppState>()?;
        let row = seasons_repo::create(
            &state.db,
            club,
            input.name.trim(),
            input.starts_at,
            input.ends_at,
        )
        .await?;
        Ok(service::to_season(row))
    }

    /// Claim a completed weekly quest, banking its XP into the active season pass.
    async fn claim_quest(&self, ctx: &Context<'_>, code: String) -> Result<QuestProgress> {
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        Ok(service::claim_quest(&state.db, user_id, &code).await?)
    }
}
