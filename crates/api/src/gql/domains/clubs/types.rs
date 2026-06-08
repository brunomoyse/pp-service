use async_graphql::{SimpleObject, ID};
use chrono::{DateTime, Utc};

#[derive(SimpleObject, Clone)]
pub struct Club {
    pub id: ID,
    pub name: String,
    pub city: Option<String>,
    pub postal_code: Option<String>,
    /// Province slug derived from the postal code (e.g. "liege", "antwerp").
    /// Stable i18n key — localize client-side, don't display raw.
    pub province: Option<String>,
}

impl From<infra::models::ClubRow> for Club {
    fn from(row: infra::models::ClubRow) -> Self {
        Self {
            id: row.id.into(),
            name: row.name,
            city: row.city,
            postal_code: row.postal_code,
            province: row.province,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct ClubTable {
    pub id: ID,
    pub club_id: ID,
    pub table_number: i32,
    pub max_seats: i32,
    pub is_active: bool,
    pub is_assigned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
