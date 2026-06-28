use async_graphql::dataloader::DataLoader;
use async_graphql::{ComplexObject, Context, Enum, Result, SimpleObject, ID};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::gql::common::helpers::display_name_from_user;
use crate::gql::loaders::{ClubPlayerLoader, UserLoader};
use infra::models::TournamentActivityLogRow;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ActivityEventCategory {
    Clock,
    Registration,
    Seating,
    Entry,
    Result,
    Tournament,
}

impl From<String> for ActivityEventCategory {
    fn from(s: String) -> Self {
        match s.as_str() {
            "clock" => Self::Clock,
            "registration" => Self::Registration,
            "seating" => Self::Seating,
            "entry" => Self::Entry,
            "result" => Self::Result,
            "tournament" => Self::Tournament,
            _ => Self::Tournament,
        }
    }
}

impl ActivityEventCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Clock => "clock",
            Self::Registration => "registration",
            Self::Seating => "seating",
            Self::Entry => "entry",
            Self::Result => "result",
            Self::Tournament => "tournament",
        }
    }
}

#[derive(SimpleObject, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[graphql(complex)]
pub struct ActivityLogEntry {
    pub id: ID,
    pub tournament_id: ID,
    pub event_category: ActivityEventCategory,
    pub event_action: String,
    pub actor_id: Option<ID>,
    pub subject_id: Option<ID>,
    pub event_time: DateTime<Utc>,
    pub metadata: async_graphql::Json<serde_json::Value>,
}

#[ComplexObject]
impl ActivityLogEntry {
    /// Display name of the actor: the user (manager or player) who performed the
    /// action. `null` for system-generated events with no actor.
    async fn actor_name(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        let Some(actor_id) = &self.actor_id else {
            return Ok(None);
        };
        let Ok(uuid) = Uuid::parse_str(actor_id.as_str()) else {
            return Ok(None);
        };
        let loader = ctx.data::<DataLoader<UserLoader>>()?;
        Ok(loader
            .load_one(uuid)
            .await?
            .map(|u| display_name_from_user(&u)))
    }

    /// Display name of the subject: the player the action targeted. Resolves an
    /// app user via `subject_id`, or an account-less roster player via
    /// `metadata.club_player_id`. `null` when the action has no player subject
    /// (e.g. a status change targets the tournament itself).
    async fn subject_name(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        if let Some(subject_id) = &self.subject_id {
            if let Ok(uuid) = Uuid::parse_str(subject_id.as_str()) {
                let loader = ctx.data::<DataLoader<UserLoader>>()?;
                return Ok(loader
                    .load_one(uuid)
                    .await?
                    .map(|u| display_name_from_user(&u)));
            }
        }

        if let Some(club_player_id) = self
            .metadata
            .0
            .get("club_player_id")
            .and_then(|v| v.as_str())
        {
            if let Ok(uuid) = Uuid::parse_str(club_player_id) {
                let loader = ctx.data::<DataLoader<ClubPlayerLoader>>()?;
                return Ok(loader.load_one(uuid).await?.map(|r| r.display_name));
            }
        }

        Ok(None)
    }
}

impl From<TournamentActivityLogRow> for ActivityLogEntry {
    fn from(row: TournamentActivityLogRow) -> Self {
        Self {
            id: row.id.into(),
            tournament_id: row.tournament_id.into(),
            event_category: ActivityEventCategory::from(row.event_category),
            event_action: row.event_action,
            actor_id: row.actor_id.map(|id| id.into()),
            subject_id: row.subject_id.map(|id| id.into()),
            event_time: row.event_time,
            metadata: async_graphql::Json(row.metadata),
        }
    }
}
