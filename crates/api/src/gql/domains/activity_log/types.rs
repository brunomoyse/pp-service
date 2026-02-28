use async_graphql::{Enum, SimpleObject, ID};
use chrono::{DateTime, Utc};

use infra::models::TournamentActivityLogRow;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
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

#[derive(SimpleObject, Clone, Debug)]
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
