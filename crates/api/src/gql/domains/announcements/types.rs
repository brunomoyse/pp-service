use async_graphql::{Enum, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};

/// Who an announcement reaches. GraphQL wire names are `TOURNAMENT` / `CLUB` /
/// `PLATFORM`.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum AnnouncementScope {
    /// Players registered for one tournament.
    Tournament,
    /// A club's roster of app users.
    Club,
    /// Every player on the platform (admin only).
    Platform,
}

impl AnnouncementScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            AnnouncementScope::Tournament => "tournament",
            AnnouncementScope::Club => "club",
            AnnouncementScope::Platform => "platform",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "club" => AnnouncementScope::Club,
            "platform" => AnnouncementScope::Platform,
            _ => AnnouncementScope::Tournament,
        }
    }
}

/// An announcement authored by a manager/admin. Persisted (this is the player
/// app's in-app feed) and also pushed to its audience on creation.
#[derive(SimpleObject, Clone, Debug)]
pub struct Announcement {
    pub id: ID,
    pub scope: AnnouncementScope,
    pub club_id: Option<ID>,
    pub tournament_id: Option<ID>,
    pub title: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

impl From<infra::models::AnnouncementRow> for Announcement {
    fn from(row: infra::models::AnnouncementRow) -> Self {
        Self {
            id: row.id.into(),
            scope: AnnouncementScope::from_db(&row.scope),
            club_id: row.club_id.map(Into::into),
            tournament_id: row.tournament_id.map(Into::into),
            title: row.title,
            body: row.body,
            created_at: row.created_at,
        }
    }
}

/// Author input for a new announcement. `clubId` is required for the `CLUB`
/// scope; `tournamentId` is required for the `TOURNAMENT` scope (its club is
/// derived from the tournament). Both are ignored for `PLATFORM`.
#[derive(InputObject)]
pub struct CreateAnnouncementInput {
    pub scope: AnnouncementScope,
    pub club_id: Option<ID>,
    pub tournament_id: Option<ID>,
    pub title: String,
    pub body: String,
}
