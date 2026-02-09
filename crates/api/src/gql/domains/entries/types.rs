use async_graphql::{Enum, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum EntryType {
    Initial,
    Rebuy,
    ReEntry,
    Addon,
}

impl From<String> for EntryType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "initial" => EntryType::Initial,
            "rebuy" => EntryType::Rebuy,
            "re_entry" => EntryType::ReEntry,
            "addon" => EntryType::Addon,
            _ => EntryType::Initial,
        }
    }
}

impl From<EntryType> for String {
    fn from(e: EntryType) -> Self {
        match e {
            EntryType::Initial => "initial".to_string(),
            EntryType::Rebuy => "rebuy".to_string(),
            EntryType::ReEntry => "re_entry".to_string(),
            EntryType::Addon => "addon".to_string(),
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct TournamentEntry {
    pub id: ID,
    pub tournament_id: ID,
    pub user_id: ID,
    pub entry_type: EntryType,
    pub amount_cents: i32,
    pub chips_received: Option<i32>,
    pub recorded_by: Option<ID>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<infra::models::TournamentEntryRow> for TournamentEntry {
    fn from(row: infra::models::TournamentEntryRow) -> Self {
        Self {
            id: row.id.into(),
            tournament_id: row.tournament_id.into(),
            user_id: row.user_id.into(),
            entry_type: EntryType::from(row.entry_type),
            amount_cents: row.amount_cents,
            chips_received: row.chips_received,
            recorded_by: row.recorded_by.map(|id| id.into()),
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct TournamentEntryStats {
    pub tournament_id: ID,
    pub total_entries: i32,
    pub total_amount_cents: i32,
    pub unique_players: i32,
    pub initial_count: i32,
    pub rebuy_count: i32,
    pub re_entry_count: i32,
    pub addon_count: i32,
}

#[derive(InputObject)]
pub struct AddTournamentEntryInput {
    pub tournament_id: ID,
    pub user_id: ID,
    pub entry_type: EntryType,
    pub amount_cents: Option<i32>,
    pub chips_received: Option<i32>,
    pub notes: Option<String>,
}
