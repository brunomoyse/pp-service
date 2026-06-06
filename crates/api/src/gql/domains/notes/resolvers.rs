use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::auth::permissions::require_pro;
use crate::features::{require_feature, Feature};
use crate::gql::domains::identity::types::RegisteredPlayer;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::models::{PlayerNoteRow, RegisteredPlayerRow};
use infra::repos::player_notes;

use super::service;
use super::types::{
    AddPlayerNoteTagInput, AddShowdownObservationInput, FieldPlayerNote, PlayerNote, PlayerNoteTag,
    ShowdownObservation, UpsertPlayerNoteInput,
};

fn author_id(ctx: &Context<'_>) -> Result<Uuid> {
    let claims = ctx.data::<Claims>()?;
    Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")
}

#[derive(Default)]
pub struct NotesQuery;

#[Object]
impl NotesQuery {
    /// The current user's note on a subject (roster entry), if any.
    async fn player_note(
        &self,
        ctx: &Context<'_>,
        subject_registered_player_id: ID,
    ) -> Result<Option<PlayerNote>> {
        require_feature(Feature::Notes)?;
        let author = author_id(ctx)?;
        let subject =
            Uuid::parse_str(subject_registered_player_id.as_str()).gql_err("Invalid subject ID")?;

        let state = ctx.data::<AppState>()?;
        let row = player_notes::get_for_subject(&state.db, author, subject).await?;
        Ok(row.map(PlayerNote::from))
    }

    /// All of the current user's notes.
    async fn my_player_notes(&self, ctx: &Context<'_>) -> Result<Vec<PlayerNote>> {
        require_feature(Feature::Notes)?;
        let author = author_id(ctx)?;
        let state = ctx.data::<AppState>()?;
        let rows = player_notes::list_for_author(&state.db, author).await?;
        Ok(rows.into_iter().map(PlayerNote::from).collect())
    }

    /// Pre-game prep: everyone registered for a tournament, paired with the
    /// viewer's own note on them. The Pro demo moment. Pro only.
    async fn tournament_field_notes(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<Vec<FieldPlayerNote>> {
        require_feature(Feature::Notes)?;
        let user = require_pro(ctx).await?;
        let author = Uuid::parse_str(user.id.as_str()).gql_err("Invalid user ID")?;
        let tid = Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let state = ctx.data::<AppState>()?;
        let rows = player_notes::field_with_notes(&state.db, tid, author).await?;

        let field = rows
            .into_iter()
            .map(|r| {
                let rp = RegisteredPlayerRow {
                    id: r.rp_id,
                    club_id: r.rp_club_id,
                    display_name: r.rp_display_name,
                    app_user_id: r.rp_app_user_id,
                    created_at: r.rp_created_at,
                    updated_at: r.rp_updated_at,
                };
                let note = r.pn_id.map(|id| {
                    PlayerNote::from(PlayerNoteRow {
                        id,
                        author_app_user_id: author,
                        subject_registered_player_id: r.rp_id,
                        body: r.pn_body.unwrap_or_default(),
                        style: r.pn_style,
                        created_at: r.pn_created_at.unwrap_or(r.rp_created_at),
                        updated_at: r.pn_updated_at.unwrap_or(r.rp_updated_at),
                    })
                });
                FieldPlayerNote {
                    registered_player: RegisteredPlayer::from(rp),
                    note,
                }
            })
            .collect();
        Ok(field)
    }
}

#[derive(Default)]
pub struct NotesMutation;

#[Object]
impl NotesMutation {
    /// Create or update the current user's note on a subject.
    async fn upsert_player_note(
        &self,
        ctx: &Context<'_>,
        input: UpsertPlayerNoteInput,
    ) -> Result<PlayerNote> {
        require_feature(Feature::Notes)?;
        let author = author_id(ctx)?;
        let subject = Uuid::parse_str(input.subject_registered_player_id.as_str())
            .gql_err("Invalid subject ID")?;

        let state = ctx.data::<AppState>()?;
        let row = service::upsert_note(
            &state.db,
            author,
            subject,
            input.body.as_deref(),
            input.style,
        )
        .await?;
        Ok(PlayerNote::from(row))
    }

    /// Delete the current user's note (and its tags/showdowns via cascade).
    async fn delete_player_note(&self, ctx: &Context<'_>, note_id: ID) -> Result<bool> {
        require_feature(Feature::Notes)?;
        let author = author_id(ctx)?;
        let id = Uuid::parse_str(note_id.as_str()).gql_err("Invalid note ID")?;
        let state = ctx.data::<AppState>()?;
        Ok(player_notes::delete_owned(&state.db, author, id).await?)
    }

    async fn add_player_note_tag(
        &self,
        ctx: &Context<'_>,
        input: AddPlayerNoteTagInput,
    ) -> Result<PlayerNoteTag> {
        require_feature(Feature::Notes)?;
        let author = author_id(ctx)?;
        let note_id = Uuid::parse_str(input.note_id.as_str()).gql_err("Invalid note ID")?;
        let state = ctx.data::<AppState>()?;
        let row = service::add_tag(&state.db, author, note_id, input.kind, &input.tag).await?;
        Ok(PlayerNoteTag::from(row))
    }

    async fn remove_player_note_tag(
        &self,
        ctx: &Context<'_>,
        note_id: ID,
        tag_id: ID,
    ) -> Result<bool> {
        require_feature(Feature::Notes)?;
        let author = author_id(ctx)?;
        let note_uuid = Uuid::parse_str(note_id.as_str()).gql_err("Invalid note ID")?;
        let tag_uuid = Uuid::parse_str(tag_id.as_str()).gql_err("Invalid tag ID")?;
        let state = ctx.data::<AppState>()?;
        Ok(service::remove_tag(&state.db, author, note_uuid, tag_uuid).await?)
    }

    async fn add_showdown_observation(
        &self,
        ctx: &Context<'_>,
        input: AddShowdownObservationInput,
    ) -> Result<ShowdownObservation> {
        require_feature(Feature::Notes)?;
        let author = author_id(ctx)?;
        let note_id = Uuid::parse_str(input.note_id.as_str()).gql_err("Invalid note ID")?;
        let tournament_id = match input.tournament_id {
            Some(t) => Some(Uuid::parse_str(t.as_str()).gql_err("Invalid tournament ID")?),
            None => None,
        };
        let state = ctx.data::<AppState>()?;
        let row = service::add_showdown(
            &state.db,
            author,
            note_id,
            tournament_id,
            &input.description,
        )
        .await?;
        Ok(ShowdownObservation::from(row))
    }
}
