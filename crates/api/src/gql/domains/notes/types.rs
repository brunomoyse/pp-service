use async_graphql::{ComplexObject, Context, Enum, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};

use crate::state::AppState;
use infra::repos::player_notes;

/// Player-style quadrant.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum PlayerStyle {
    Tag,
    Lag,
    TightPassive,
    LoosePassive,
}

impl PlayerStyle {
    pub fn as_db(self) -> &'static str {
        match self {
            PlayerStyle::Tag => "TAG",
            PlayerStyle::Lag => "LAG",
            PlayerStyle::TightPassive => "TP",
            PlayerStyle::LoosePassive => "LP",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "TAG" => Some(PlayerStyle::Tag),
            "LAG" => Some(PlayerStyle::Lag),
            "TP" => Some(PlayerStyle::TightPassive),
            "LP" => Some(PlayerStyle::LoosePassive),
            _ => None,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum NoteTagKind {
    Tag,
    Tell,
}

impl NoteTagKind {
    pub fn as_db(self) -> &'static str {
        match self {
            NoteTagKind::Tag => "tag",
            NoteTagKind::Tell => "tell",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "tell" => NoteTagKind::Tell,
            _ => NoteTagKind::Tag,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct PlayerNoteTag {
    pub id: ID,
    pub kind: NoteTagKind,
    pub tag: String,
}

impl From<infra::models::PlayerNoteTagRow> for PlayerNoteTag {
    fn from(r: infra::models::PlayerNoteTagRow) -> Self {
        Self {
            id: r.id.into(),
            kind: NoteTagKind::from_db(&r.kind),
            tag: r.tag,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct ShowdownObservation {
    pub id: ID,
    pub tournament_id: Option<ID>,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

impl From<infra::models::ShowdownObservationRow> for ShowdownObservation {
    fn from(r: infra::models::ShowdownObservationRow) -> Self {
        Self {
            id: r.id.into(),
            tournament_id: r.tournament_id.map(Into::into),
            description: r.description,
            created_at: r.created_at,
        }
    }
}

/// A private note on one subject. Only ever returned to its author, so its
/// children (tags, showdowns) can be loaded by note id alone.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct PlayerNote {
    pub id: ID,
    pub subject_club_player_id: ID,
    pub body: String,
    pub style: Option<PlayerStyle>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<infra::models::PlayerNoteRow> for PlayerNote {
    fn from(r: infra::models::PlayerNoteRow) -> Self {
        Self {
            id: r.id.into(),
            subject_club_player_id: r.subject_club_player_id.into(),
            body: r.body,
            style: r.style.as_deref().and_then(PlayerStyle::from_db),
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[ComplexObject]
impl PlayerNote {
    /// The roster entry this note is about.
    async fn subject(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Option<crate::gql::domains::identity::types::ClubPlayer>> {
        let state = ctx.data::<AppState>()?;
        let subject_id = uuid::Uuid::parse_str(self.subject_club_player_id.as_str())?;
        let row = infra::repos::club_players::get_by_id(&state.db, subject_id).await?;
        Ok(row.map(crate::gql::domains::identity::types::ClubPlayer::from))
    }

    async fn tags(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<PlayerNoteTag>> {
        let state = ctx.data::<AppState>()?;
        let note_id = uuid::Uuid::parse_str(self.id.as_str())?;
        let rows = player_notes::list_tags(&state.db, note_id).await?;
        Ok(rows.into_iter().map(PlayerNoteTag::from).collect())
    }

    async fn showdown_observations(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<ShowdownObservation>> {
        let state = ctx.data::<AppState>()?;
        let note_id = uuid::Uuid::parse_str(self.id.as_str())?;
        let rows = player_notes::list_showdowns(&state.db, note_id).await?;
        Ok(rows.into_iter().map(ShowdownObservation::from).collect())
    }
}

/// A player in tonight's field, paired with the viewer's note on them (if any).
/// The pre-game-prep payload: surface who's registered and what you know on them.
#[derive(SimpleObject, Clone, Debug)]
pub struct FieldPlayerNote {
    pub club_player: crate::gql::domains::identity::types::ClubPlayer,
    pub note: Option<PlayerNote>,
}

#[derive(InputObject)]
pub struct UpsertPlayerNoteInput {
    pub subject_club_player_id: ID,
    pub body: Option<String>,
    pub style: Option<PlayerStyle>,
}

#[derive(InputObject)]
pub struct AddPlayerNoteTagInput {
    pub note_id: ID,
    pub kind: NoteTagKind,
    pub tag: String,
}

#[derive(InputObject)]
pub struct AddShowdownObservationInput {
    pub note_id: ID,
    pub tournament_id: Option<ID>,
    pub description: String,
}
