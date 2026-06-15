use async_graphql::{ComplexObject, Context, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};

use crate::gql::domains::clubs::types::Club;
use crate::gql::error::ResultExt;
use crate::state::AppState;

/// A club roster entry. Exists for everyone a club has registered, whether or
/// not they are an onboarded app user. `app_user_id` is set once claimed.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct ClubPlayer {
    pub id: ID,
    pub club_id: ID,
    pub display_name: String,
    /// Given name(s). Null for legacy / bulk-imported single-field entries.
    pub first_name: Option<String>,
    /// Family name. Null for legacy / bulk-imported single-field entries.
    pub last_name: Option<String>,
    /// The linked app user, set once the roster entry has been claimed.
    pub app_user_id: Option<ID>,
    /// Whether this roster entry is linked to an onboarded app user.
    pub is_claimed: bool,
    /// Whether this roster entry is active. Archived entries (`false`) are
    /// hidden from the roster but keep their historical references.
    pub is_active: bool,
}

impl From<infra::models::ClubPlayerRow> for ClubPlayer {
    fn from(row: infra::models::ClubPlayerRow) -> Self {
        let is_claimed = row.app_user_id.is_some();
        Self {
            id: row.id.into(),
            club_id: row.club_id.into(),
            display_name: row.display_name,
            first_name: row.first_name,
            last_name: row.last_name,
            app_user_id: row.app_user_id.map(Into::into),
            is_claimed,
            is_active: row.is_active,
        }
    }
}

#[ComplexObject]
impl ClubPlayer {
    /// The club this roster entry belongs to.
    async fn club(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Club>> {
        let state = ctx.data::<AppState>()?;
        let club_id = uuid::Uuid::parse_str(self.club_id.as_str()).gql_err("Invalid club ID")?;
        let row = infra::repos::clubs::get_by_id(&state.db, club_id).await?;
        Ok(row.map(Club::from))
    }

    /// When the linked app user was last active (login / token refresh). Null for
    /// roster-only entries (never claimed) and for users predating activity
    /// tracking.
    async fn last_seen_at(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Option<DateTime<Utc>>> {
        let Some(app_user_id) = self.app_user_id.as_ref() else {
            return Ok(None);
        };
        let state = ctx.data::<AppState>()?;
        let user_id = uuid::Uuid::parse_str(app_user_id.as_str()).gql_err("Invalid user ID")?;
        let last_seen = infra::repos::users::get_last_seen_at(&state.db, user_id).await?;
        Ok(last_seen)
    }
}

/// Manager input to add a person who is not (yet) an app user to the roster.
#[derive(InputObject)]
pub struct CreateClubPlayerInput {
    pub club_id: ID,
    pub first_name: String,
    #[graphql(default)]
    pub last_name: String,
}

/// Player input to claim an unclaimed roster entry as their own.
#[derive(InputObject)]
pub struct ClaimClubPlayerInput {
    pub club_player_id: ID,
}

/// Manager input to rename a roster entry (structured first/last name).
#[derive(InputObject)]
pub struct UpdateClubPlayerInput {
    pub id: ID,
    pub first_name: String,
    #[graphql(default)]
    pub last_name: String,
}

/// Manager input to archive (soft-delete) or restore a roster entry.
#[derive(InputObject)]
pub struct ArchiveClubPlayerInput {
    pub id: ID,
    /// `false` archives the entry; `true` restores it. Defaults to archive.
    #[graphql(default = false)]
    pub is_active: bool,
}

// ---------------------------------------------------------------------------
// Bulk import (Excel/CSV) — AI formatting + bulk roster creation.
// ---------------------------------------------------------------------------

/// One parsed spreadsheet row, normalized by the AI into a clean display name.
#[derive(SimpleObject, Clone, Debug)]
pub struct ImportCandidate {
    /// Index of the originating row in the submitted `rows` array.
    pub source_row_index: i32,
    /// The cleaned, ready-to-import player display name.
    pub display_name: String,
}

/// Manager input: the parsed spreadsheet to normalize via AI.
#[derive(InputObject)]
pub struct FormatRosterImportInput {
    pub club_id: ID,
    /// Column headers from the spreadsheet (first row).
    pub headers: Vec<String>,
    /// Data rows; each inner vec aligns positionally with `headers`.
    pub rows: Vec<Vec<String>>,
}

/// Manager input: the confirmed names to insert into the roster.
#[derive(InputObject)]
pub struct CreateClubPlayersBulkInput {
    pub club_id: ID,
    pub display_names: Vec<String>,
}

/// A row that was not inserted during a bulk import, with the reason.
#[derive(SimpleObject, Clone, Debug)]
pub struct SkippedRow {
    pub display_name: String,
    pub reason: String,
}

/// Outcome of a bulk roster import.
#[derive(SimpleObject, Clone, Debug)]
pub struct BulkRosterResult {
    pub created: Vec<ClubPlayer>,
    pub skipped: Vec<SkippedRow>,
}
