use async_graphql::dataloader::DataLoader;
use async_graphql::{ComplexObject, Context, Enum, InputObject, Result, SimpleObject, ID};
use chrono::{DateTime, Utc};

use crate::gql::domains::clubs::types::Club;
use crate::gql::domains::registrations::types::TournamentRegistration;
use crate::gql::domains::tournaments::recurrence::RecurrenceFrequency;
use crate::gql::error::ResultExt;
use crate::gql::loaders::ClubLoader;

// Tournament status enums

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum TournamentStatus {
    Upcoming,
    InProgress,
    Completed,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum TournamentLiveStatus {
    NotStarted,
    RegistrationOpen,
    LateRegistration,
    InProgress,
    Break,
    FinalTable,
    Finished,
}

impl From<TournamentStatus> for infra::repos::tournaments::TournamentStatus {
    fn from(status: TournamentStatus) -> Self {
        match status {
            TournamentStatus::Upcoming => infra::repos::tournaments::TournamentStatus::Upcoming,
            TournamentStatus::InProgress => infra::repos::tournaments::TournamentStatus::InProgress,
            TournamentStatus::Completed => infra::repos::tournaments::TournamentStatus::Completed,
        }
    }
}

impl From<infra::repos::tournaments::TournamentStatus> for TournamentStatus {
    fn from(status: infra::repos::tournaments::TournamentStatus) -> Self {
        match status {
            infra::repos::tournaments::TournamentStatus::Upcoming => TournamentStatus::Upcoming,
            infra::repos::tournaments::TournamentStatus::InProgress => TournamentStatus::InProgress,
            infra::repos::tournaments::TournamentStatus::Completed => TournamentStatus::Completed,
        }
    }
}

impl From<String> for TournamentLiveStatus {
    fn from(status: String) -> Self {
        match status.as_str() {
            "not_started" => TournamentLiveStatus::NotStarted,
            "registration_open" => TournamentLiveStatus::RegistrationOpen,
            "late_registration" => TournamentLiveStatus::LateRegistration,
            "in_progress" => TournamentLiveStatus::InProgress,
            "break" => TournamentLiveStatus::Break,
            "final_table" => TournamentLiveStatus::FinalTable,
            "finished" => TournamentLiveStatus::Finished,
            _ => TournamentLiveStatus::NotStarted, // Default to not_started for invalid statuses
        }
    }
}

impl From<Option<String>> for TournamentLiveStatus {
    fn from(status: Option<String>) -> Self {
        match status {
            Some(s) => TournamentLiveStatus::from(s),
            None => TournamentLiveStatus::NotStarted, // Default to not_started if no status specified
        }
    }
}

impl From<TournamentLiveStatus> for String {
    fn from(status: TournamentLiveStatus) -> Self {
        match status {
            TournamentLiveStatus::NotStarted => "not_started".to_string(),
            TournamentLiveStatus::RegistrationOpen => "registration_open".to_string(),
            TournamentLiveStatus::LateRegistration => "late_registration".to_string(),
            TournamentLiveStatus::InProgress => "in_progress".to_string(),
            TournamentLiveStatus::Break => "break".to_string(),
            TournamentLiveStatus::FinalTable => "final_table".to_string(),
            TournamentLiveStatus::Finished => "finished".to_string(),
        }
    }
}

impl From<infra::repos::tournaments::TournamentLiveStatus> for TournamentLiveStatus {
    fn from(status: infra::repos::tournaments::TournamentLiveStatus) -> Self {
        match status {
            infra::repos::tournaments::TournamentLiveStatus::NotStarted => {
                TournamentLiveStatus::NotStarted
            }
            infra::repos::tournaments::TournamentLiveStatus::RegistrationOpen => {
                TournamentLiveStatus::RegistrationOpen
            }
            infra::repos::tournaments::TournamentLiveStatus::LateRegistration => {
                TournamentLiveStatus::LateRegistration
            }
            infra::repos::tournaments::TournamentLiveStatus::InProgress => {
                TournamentLiveStatus::InProgress
            }
            infra::repos::tournaments::TournamentLiveStatus::Break => TournamentLiveStatus::Break,
            infra::repos::tournaments::TournamentLiveStatus::FinalTable => {
                TournamentLiveStatus::FinalTable
            }
            infra::repos::tournaments::TournamentLiveStatus::Finished => {
                TournamentLiveStatus::Finished
            }
        }
    }
}

impl From<TournamentLiveStatus> for infra::repos::tournaments::TournamentLiveStatus {
    fn from(status: TournamentLiveStatus) -> Self {
        match status {
            TournamentLiveStatus::NotStarted => {
                infra::repos::tournaments::TournamentLiveStatus::NotStarted
            }
            TournamentLiveStatus::RegistrationOpen => {
                infra::repos::tournaments::TournamentLiveStatus::RegistrationOpen
            }
            TournamentLiveStatus::LateRegistration => {
                infra::repos::tournaments::TournamentLiveStatus::LateRegistration
            }
            TournamentLiveStatus::InProgress => {
                infra::repos::tournaments::TournamentLiveStatus::InProgress
            }
            TournamentLiveStatus::Break => infra::repos::tournaments::TournamentLiveStatus::Break,
            TournamentLiveStatus::FinalTable => {
                infra::repos::tournaments::TournamentLiveStatus::FinalTable
            }
            TournamentLiveStatus::Finished => {
                infra::repos::tournaments::TournamentLiveStatus::Finished
            }
        }
    }
}

// Clock status enum

#[derive(Enum, Copy, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ClockStatus {
    Stopped,
    Running,
    Paused,
}

impl From<infra::repos::tournament_clock::ClockStatus> for ClockStatus {
    fn from(status: infra::repos::tournament_clock::ClockStatus) -> Self {
        match status {
            infra::repos::tournament_clock::ClockStatus::Stopped => ClockStatus::Stopped,
            infra::repos::tournament_clock::ClockStatus::Running => ClockStatus::Running,
            infra::repos::tournament_clock::ClockStatus::Paused => ClockStatus::Paused,
        }
    }
}

impl From<ClockStatus> for infra::repos::tournament_clock::ClockStatus {
    fn from(status: ClockStatus) -> Self {
        match status {
            ClockStatus::Stopped => infra::repos::tournament_clock::ClockStatus::Stopped,
            ClockStatus::Running => infra::repos::tournament_clock::ClockStatus::Running,
            ClockStatus::Paused => infra::repos::tournament_clock::ClockStatus::Paused,
        }
    }
}

// Core tournament objects

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum BountyType {
    None,
    Fixed,
    Progressive,
}

impl From<String> for BountyType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "fixed" => BountyType::Fixed,
            "progressive" => BountyType::Progressive,
            _ => BountyType::None,
        }
    }
}

impl From<BountyType> for String {
    fn from(b: BountyType) -> Self {
        match b {
            BountyType::None => "none",
            BountyType::Fixed => "fixed",
            BountyType::Progressive => "progressive",
        }
        .to_string()
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(complex)]
pub struct Tournament {
    pub id: ID,
    pub title: String,
    pub description: Option<String>,
    pub club_id: ID,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub buy_in_cents: i32,
    pub rake_cents: i32,
    pub seat_cap: Option<i32>,
    pub starting_stack: Option<i32>, // Default chips granted on the initial buy-in
    pub status: TournamentStatus,    // Calculated: UPCOMING, LIVE, COMPLETED
    pub live_status: TournamentLiveStatus, // Direct from DB: NOT_STARTED, IN_PROGRESS, FINISHED, etc.
    pub early_bird_bonus_chips: Option<i32>, // Extra chips for players present at tournament start
    pub level_two_bonus_chips: Option<i32>, // Extra chips for players still seated at end of L2
    pub voucher_value_cents: i32,          // Mandatory drink voucher (excluded from prize pool)
    pub rebuy_max: Option<i32>,            // Max rebuys allowed (flyer display)
    pub addon_chips: Option<i32>,          // Add-on chip amount (flyer display)
    pub addon_price_cents: Option<i32>,    // Add-on price in cents (flyer display)
    pub late_registration_level: Option<i32>, // Blind level until which late registration stays open
    pub bounty_type: BountyType,              // none | fixed | progressive (PKO)
    pub bounty_amount_cents: i32,             // Bounty slice of each buy-in / rebuy / re-entry
    pub leaderboard_config_id: Option<ID>,    // Optional league tag (feeds `tagged` leagues)
    pub series_id: Option<ID>,                // Multi-day series (NULL = standalone single-day)
    pub flight_label: Option<String>,         // e.g. "Day 1A", "Day 2"
    pub is_final_day: bool,                   // Series final day: results, points, aggregate pool
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(SimpleObject, Clone, serde::Serialize, serde::Deserialize)]
pub struct TournamentStructure {
    pub id: ID,
    pub tournament_id: ID,
    pub level_number: i32,
    pub small_blind: i32,
    pub big_blind: i32,
    pub ante: i32,
    pub duration_minutes: i32,
    pub is_break: bool,
    pub break_duration_minutes: Option<i32>,
}

impl From<infra::models::TournamentStructureRow> for TournamentStructure {
    fn from(row: infra::models::TournamentStructureRow) -> Self {
        Self {
            id: row.id.into(),
            tournament_id: row.tournament_id.into(),
            level_number: row.level_number,
            small_blind: row.small_blind,
            big_blind: row.big_blind,
            ante: row.ante,
            duration_minutes: row.duration_minutes,
            is_break: row.is_break,
            break_duration_minutes: row.break_duration_minutes,
        }
    }
}

impl From<infra::models::TournamentRow> for Tournament {
    fn from(row: infra::models::TournamentRow) -> Self {
        let status = row.calculate_status().into();
        Self {
            id: row.id.into(),
            title: row.name,
            description: row.description,
            club_id: row.club_id.into(),
            start_time: row.start_time,
            end_time: row.end_time,
            buy_in_cents: row.buy_in_cents,
            rake_cents: row.rake_cents,
            seat_cap: row.seat_cap,
            starting_stack: row.starting_stack,
            status,
            live_status: row.live_status.into(),
            early_bird_bonus_chips: row.early_bird_bonus_chips,
            level_two_bonus_chips: row.level_two_bonus_chips,
            voucher_value_cents: row.voucher_value_cents,
            rebuy_max: row.rebuy_max,
            addon_chips: row.addon_chips,
            addon_price_cents: row.addon_price_cents,
            late_registration_level: row.late_registration_level,
            bounty_type: BountyType::from(row.bounty_type),
            bounty_amount_cents: row.bounty_amount_cents,
            leaderboard_config_id: row.leaderboard_config_id.map(|id| id.into()),
            series_id: row.series_id.map(|id| id.into()),
            flight_label: row.flight_label,
            is_final_day: row.is_final_day,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(SimpleObject, Clone, serde::Serialize, serde::Deserialize)]
pub struct TournamentClock {
    pub id: ID,
    pub tournament_id: ID,
    pub status: ClockStatus,
    pub current_level: i32,
    pub time_remaining_seconds: Option<i64>, // Calculated field
    pub level_started_at: Option<DateTime<Utc>>,
    pub level_end_time: Option<DateTime<Utc>>,
    pub total_pause_duration_seconds: i64, // Calculated field
    pub auto_advance: bool,
    pub current_structure: Option<TournamentStructure>,
    pub next_structure: Option<TournamentStructure>,
    // Additional fields for real-time updates (previously in ClockUpdate)
    pub small_blind: Option<i32>,
    pub big_blind: Option<i32>,
    pub ante: Option<i32>,
    pub is_break: Option<bool>,
    pub level_duration_minutes: Option<i32>,
}

#[ComplexObject]
impl Tournament {
    async fn club(&self, ctx: &Context<'_>) -> Result<Club> {
        let loader = ctx.data::<DataLoader<ClubLoader>>()?;
        let club_uuid = uuid::Uuid::parse_str(self.club_id.as_str()).gql_err("Invalid club ID")?;

        match loader
            .load_one(club_uuid)
            .await
            .gql_err("Loading club failed")?
        {
            Some(row) => Ok(row.into()),
            None => Err(async_graphql::Error::new("Club not found")),
        }
    }

    async fn structure(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<TournamentStructure>> {
        use crate::state::AppState;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            uuid::Uuid::parse_str(self.id.as_str()).gql_err("Invalid tournament ID")?;

        let structures =
            infra::repos::tournament_clock::get_all_structures(&state.db, tournament_id).await?;

        Ok(structures
            .into_iter()
            .map(TournamentStructure::from)
            .collect())
    }

    async fn clock(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<TournamentClock>> {
        use crate::state::AppState;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            uuid::Uuid::parse_str(self.id.as_str()).gql_err("Invalid tournament ID")?;

        super::clock::load_tournament_clock(&state.db, tournament_id).await
    }

    /// Number of players taking part (excludes cancellations and no-shows).
    async fn registration_count(&self, ctx: &Context<'_>) -> async_graphql::Result<i64> {
        use crate::state::AppState;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            uuid::Uuid::parse_str(self.id.as_str()).gql_err("Invalid tournament ID")?;

        Ok(
            infra::repos::tournament_registrations::count_participants_by_tournament(
                &state.db,
                tournament_id,
            )
            .await?,
        )
    }

    async fn registrations(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<TournamentRegistration>> {
        use crate::state::AppState;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            uuid::Uuid::parse_str(self.id.as_str()).gql_err("Invalid tournament ID")?;

        let registrations =
            infra::repos::tournament_registrations::list_by_tournament(&state.db, tournament_id)
                .await?;

        Ok(registrations
            .into_iter()
            .map(TournamentRegistration::from)
            .collect())
    }
}

// Tournament input types

#[derive(InputObject)]
pub struct CreateTournamentInput {
    pub club_id: ID,
    pub name: String,
    pub description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub buy_in_cents: i32,
    pub rake_cents: Option<i32>,
    pub seat_cap: Option<i32>,
    /// Default chips a player receives on their initial buy-in.
    pub starting_stack: Option<i32>,
    pub early_bird_bonus_chips: Option<i32>,
    pub level_two_bonus_chips: Option<i32>,
    pub voucher_value_cents: Option<i32>,
    pub rebuy_max: Option<i32>,
    pub addon_chips: Option<i32>,
    pub addon_price_cents: Option<i32>,
    pub late_registration_level: Option<i32>,
    pub bounty_type: Option<BountyType>,
    pub bounty_amount_cents: Option<i32>,
    /// Optional league this tournament counts toward (feeds `tagged` leagues).
    pub leaderboard_config_id: Option<ID>,
    /// Blind structure template ID - if provided, copies levels from template
    pub template_id: Option<ID>,
    /// Custom blind structure levels - only used if template_id is not provided
    pub structure: Option<Vec<TournamentStructureInput>>,
    /// If set, this tournament repeats on the given cadence and an occurrence is
    /// created up front for each interval up to `recurrence_end_date`. Each
    /// occurrence is an independent tournament. Absent ⇒ a single tournament.
    pub recurrence_frequency: Option<RecurrenceFrequency>,
    /// Last date (inclusive) to generate occurrences for. Required when
    /// `recurrence_frequency` is set; ignored otherwise.
    pub recurrence_end_date: Option<DateTime<Utc>>,
}

#[derive(InputObject)]
pub struct UpdateTournamentInput {
    pub id: ID,
    pub name: Option<String>,
    pub description: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub buy_in_cents: Option<i32>,
    pub rake_cents: Option<i32>,
    pub seat_cap: Option<i32>,
    /// Default chips a player receives on their initial buy-in.
    pub starting_stack: Option<i32>,
    pub early_bird_bonus_chips: Option<i32>,
    pub level_two_bonus_chips: Option<i32>,
    pub voucher_value_cents: Option<i32>,
    pub rebuy_max: Option<i32>,
    pub addon_chips: Option<i32>,
    pub addon_price_cents: Option<i32>,
    pub late_registration_level: Option<i32>,
    pub bounty_type: Option<BountyType>,
    pub bounty_amount_cents: Option<i32>,
    /// Optional league this tournament counts toward (feeds `tagged` leagues).
    pub leaderboard_config_id: Option<ID>,
    /// Blind structure template ID - if provided, replaces structure with template levels
    pub template_id: Option<ID>,
    /// Custom blind structure levels - only used if template_id is not provided
    pub structure: Option<Vec<TournamentStructureInput>>,
}

#[derive(InputObject, Clone)]
pub struct TournamentStructureInput {
    pub level_number: i32,
    pub small_blind: i32,
    pub big_blind: i32,
    pub ante: i32,
    pub duration_minutes: i32,
    pub is_break: bool,
    pub break_duration_minutes: Option<i32>,
}

#[derive(InputObject)]
pub struct UpdateTournamentStatusInput {
    pub tournament_id: ID,
    pub live_status: TournamentLiveStatus,
}
