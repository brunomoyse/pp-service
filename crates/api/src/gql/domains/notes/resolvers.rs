use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::gql::domains::identity::types::ClubPlayer;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::models::{ClubPlayerRow, PlayerNoteRow};
use infra::repos::player_notes;

use super::service;
use super::types::{
    AddPlayerNoteTagInput, AddShowdownObservationInput, FieldPlayerNote, MyTableView, PlayerNote,
    PlayerNoteTag, ShowdownObservation, TableSeatNote, UpsertPlayerNoteInput,
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
        subject_club_player_id: ID,
    ) -> Result<Option<PlayerNote>> {
        let author = author_id(ctx)?;
        let subject =
            Uuid::parse_str(subject_club_player_id.as_str()).gql_err("Invalid subject ID")?;

        let state = ctx.data::<AppState>()?;
        let row = player_notes::get_for_subject(&state.db, author, subject).await?;
        Ok(row.map(PlayerNote::from))
    }

    /// All of the current user's notes.
    async fn my_player_notes(&self, ctx: &Context<'_>) -> Result<Vec<PlayerNote>> {
        let author = author_id(ctx)?;
        let state = ctx.data::<AppState>()?;
        let rows = player_notes::list_for_author(&state.db, author).await?;
        Ok(rows.into_iter().map(PlayerNote::from).collect())
    }

    /// Pre-game prep: everyone registered for a tournament, paired with the
    /// viewer's own note on them.
    async fn tournament_field_notes(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<Vec<FieldPlayerNote>> {
        let author = author_id(ctx)?;
        let tid = Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let state = ctx.data::<AppState>()?;
        let rows = player_notes::field_with_notes(&state.db, tid, author).await?;

        let field = rows
            .into_iter()
            .map(|r| {
                let rp = ClubPlayerRow {
                    id: r.rp_id,
                    club_id: r.rp_club_id,
                    display_name: r.rp_display_name,
                    // Scouting projection doesn't select the structured name parts.
                    first_name: None,
                    last_name: None,
                    app_user_id: r.rp_app_user_id,
                    // These rows are players currently in a tournament field, so
                    // they are active by construction; the scouting query does
                    // not select is_active.
                    is_active: true,
                    created_at: r.rp_created_at,
                    updated_at: r.rp_updated_at,
                };
                let note = r.pn_id.map(|id| {
                    PlayerNote::from(PlayerNoteRow {
                        id,
                        author_app_user_id: author,
                        subject_club_player_id: r.rp_id,
                        body: r.pn_body.unwrap_or_default(),
                        style: r.pn_style,
                        color: r.pn_color,
                        created_at: r.pn_created_at.unwrap_or(r.rp_created_at),
                        updated_at: r.pn_updated_at.unwrap_or(r.rp_updated_at),
                    })
                });
                FieldPlayerNote {
                    club_player: ClubPlayer::from(rp),
                    note,
                }
            })
            .collect();
        Ok(field)
    }

    /// Live prep: the players seated at the viewer's own table, each paired with
    /// the viewer's private note. Null when the viewer isn't currently seated.
    async fn my_table_notes(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<Option<MyTableView>> {
        let author = author_id(ctx)?;
        let tid = Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let state = ctx.data::<AppState>()?;
        let rows = player_notes::table_with_notes(&state.db, tid, author).await?;
        if rows.is_empty() {
            return Ok(None);
        }

        let table_number = rows[0].table_number;
        let mut my_seat_number = None;
        let mut seats = Vec::new();
        for r in rows {
            // The viewer's own seat sets the header; they aren't a tablemate.
            if r.rp_app_user_id == Some(author) {
                my_seat_number = Some(r.seat_number);
                continue;
            }
            let rp = ClubPlayerRow {
                id: r.rp_id,
                club_id: r.rp_club_id,
                display_name: r.rp_display_name,
                first_name: None,
                last_name: None,
                app_user_id: r.rp_app_user_id,
                is_active: true,
                created_at: r.rp_created_at,
                updated_at: r.rp_updated_at,
            };
            let note = r.pn_id.map(|id| {
                PlayerNote::from(PlayerNoteRow {
                    id,
                    author_app_user_id: author,
                    subject_club_player_id: r.rp_id,
                    body: r.pn_body.unwrap_or_default(),
                    style: r.pn_style,
                    color: r.pn_color,
                    created_at: r.pn_created_at.unwrap_or(r.rp_created_at),
                    updated_at: r.pn_updated_at.unwrap_or(r.rp_updated_at),
                })
            });
            seats.push(TableSeatNote {
                club_player: ClubPlayer::from(rp),
                seat_number: r.seat_number,
                stack_size: r.stack_size,
                note,
            });
        }

        Ok(Some(MyTableView {
            table_number,
            my_seat_number,
            seats,
        }))
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
        let author = author_id(ctx)?;
        let subject =
            Uuid::parse_str(input.subject_club_player_id.as_str()).gql_err("Invalid subject ID")?;

        let state = ctx.data::<AppState>()?;
        let row = service::upsert_note(
            &state.db,
            author,
            subject,
            input.body.as_deref(),
            input.style,
            input.color,
        )
        .await?;
        Ok(PlayerNote::from(row))
    }

    /// Delete the current user's note (and its tags/showdowns via cascade).
    async fn delete_player_note(&self, ctx: &Context<'_>, note_id: ID) -> Result<bool> {
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
