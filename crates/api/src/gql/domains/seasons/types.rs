use async_graphql::{ComplexObject, Context, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};

use crate::gql::domains::clubs::types::Club;
use crate::gql::error::ResultExt;
use crate::state::AppState;

/// A club's competitive season — a named time window.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct Season {
    pub id: ID,
    pub club_id: ID,
    pub name: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    /// Whether the season is currently running.
    pub is_active: bool,
}

#[ComplexObject]
impl Season {
    /// The club this season belongs to.
    async fn club(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Club>> {
        let state = ctx.data::<AppState>()?;
        let club_id = uuid::Uuid::parse_str(self.club_id.as_str()).gql_err("Invalid club ID")?;
        let row = infra::repos::clubs::get_by_id(&state.db, club_id).await?;
        Ok(row.map(Club::from))
    }
}

impl Season {
    pub fn from_row(row: infra::models::SeasonRow, now: DateTime<Utc>) -> Self {
        Self {
            id: row.id.into(),
            club_id: row.club_id.into(),
            name: row.name,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
            is_active: row.starts_at <= now && row.ends_at > now,
        }
    }
}

/// A player's progress on a season's reward track. XP is earned-only (G1).
#[derive(SimpleObject, Clone, Debug)]
pub struct SeasonPass {
    pub season_id: ID,
    pub xp: i32,
    pub tier: i32,
    /// XP accumulated inside the current tier.
    pub xp_into_tier: i32,
    /// XP required to span one tier.
    pub xp_per_tier: i32,
}

/// A weekly quest with the current player's progress against it.
#[derive(SimpleObject, Clone, Debug)]
pub struct QuestProgress {
    /// Stable catalog code; the client maps it to localized copy.
    pub code: String,
    pub target: i32,
    pub progress: i32,
    pub completed: bool,
    /// Already claimed this week (XP awarded).
    pub claimed: bool,
    pub xp_reward: i32,
}

/// A finished season's champion — the most-present player.
#[derive(SimpleObject, Clone, Debug)]
pub struct HallOfFameEntry {
    pub season_id: ID,
    pub season_name: String,
    pub ends_at: DateTime<Utc>,
    pub champion_name: String,
    /// Number of events the champion attended that season.
    pub events: i32,
}

#[derive(InputObject)]
pub struct CreateSeasonInput {
    pub club_id: ID,
    pub name: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}
