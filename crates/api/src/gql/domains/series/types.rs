use async_graphql::{ComplexObject, Context, InputObject, Result, SimpleObject, ID};
use chrono::{DateTime, Utc};

use crate::gql::domains::tournaments::types::{Tournament, TournamentStructureInput};
use crate::gql::error::ResultExt;
use crate::state::AppState;

/// A multi-day event grouping several Day-1 flights plus a final day.
#[derive(SimpleObject, Clone)]
#[graphql(complex)]
pub struct TournamentSeries {
    pub id: ID,
    pub club_id: ID,
    pub title: String,
    pub best_stack_forward: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<infra::repos::tournament_series::TournamentSeriesRow> for TournamentSeries {
    fn from(row: infra::repos::tournament_series::TournamentSeriesRow) -> Self {
        Self {
            id: row.id.into(),
            club_id: row.club_id.into(),
            title: row.title,
            best_stack_forward: row.best_stack_forward,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[ComplexObject]
impl TournamentSeries {
    /// All flights + the final day, ordered (flights first, then the final day).
    async fn flights(&self, ctx: &Context<'_>) -> Result<Vec<Tournament>> {
        let state = ctx.data::<AppState>()?;
        let series_uuid = uuid::Uuid::parse_str(self.id.as_str()).gql_err("Invalid series ID")?;
        let rows = infra::repos::tournaments::list_by_series(&state.db, series_uuid).await?;
        Ok(rows.into_iter().map(Tournament::from).collect())
    }

    /// One row per qualified player (the best stack carried to Day 2).
    async fn qualifications(&self, ctx: &Context<'_>) -> Result<Vec<FlightQualification>> {
        let state = ctx.data::<AppState>()?;
        let series_uuid = uuid::Uuid::parse_str(self.id.as_str()).gql_err("Invalid series ID")?;
        let rows = infra::repos::flight_qualifications::list_best_by_series(&state.db, series_uuid)
            .await?;
        Ok(rows.into_iter().map(FlightQualification::from).collect())
    }
}

#[derive(SimpleObject, Clone)]
pub struct FlightQualification {
    pub id: ID,
    pub series_id: ID,
    pub club_player_id: ID,
    pub from_tournament_id: ID,
    pub chip_count: i32,
    pub is_best: bool,
    pub created_at: DateTime<Utc>,
}

impl From<infra::repos::flight_qualifications::FlightQualificationRow> for FlightQualification {
    fn from(row: infra::repos::flight_qualifications::FlightQualificationRow) -> Self {
        Self {
            id: row.id.into(),
            series_id: row.series_id.into(),
            club_player_id: row.club_player_id.into(),
            from_tournament_id: row.from_tournament_id.into(),
            chip_count: row.chip_count,
            is_best: row.is_best,
            created_at: row.created_at,
        }
    }
}

// ── Inputs ──────────────────────────────────────────────────────────────

/// One flight (or the final day) in a series: a label and a start time. All
/// other config (buy-in, structure, payout) is shared at the series level.
#[derive(InputObject, Clone)]
pub struct FlightInput {
    pub label: String,
    pub start_time: DateTime<Utc>,
}

#[derive(InputObject)]
pub struct CreateTournamentSeriesInput {
    pub club_id: ID,
    pub title: String,
    pub best_stack_forward: Option<bool>,
    // Shared config applied to every flight + the final day.
    pub buy_in_cents: i32,
    pub rake_cents: Option<i32>,
    pub seat_cap: Option<i32>,
    pub late_registration_level: Option<i32>,
    /// Blind structure template applied to every flight (copied per tournament).
    pub template_id: Option<ID>,
    /// Custom blind structure (used only when template_id is absent).
    pub structure: Option<Vec<TournamentStructureInput>>,
    /// Day-1 flights, in order.
    pub flights: Vec<FlightInput>,
    /// The final day (Day 2).
    pub final_day: FlightInput,
}

#[derive(InputObject)]
pub struct SurvivorInput {
    pub club_player_id: ID,
    pub chip_count: i32,
}

#[derive(InputObject)]
pub struct CloseFlightInput {
    pub tournament_id: ID,
    pub survivors: Vec<SurvivorInput>,
}
