use sqlx::PgPool;
use uuid::Uuid;

use super::types::{NoteTagKind, PlayerStyle};
use crate::gql::error::GqlError;
use infra::models::{PlayerNoteRow, PlayerNoteTagRow, ShowdownObservationRow};
use infra::repos::player_notes;

pub async fn upsert_note(
    db: &PgPool,
    author: Uuid,
    subject: Uuid,
    body: Option<&str>,
    style: Option<PlayerStyle>,
) -> Result<PlayerNoteRow, GqlError> {
    let body = body.unwrap_or("");
    let style_db = style.map(|s| s.as_db());
    Ok(player_notes::upsert(db, author, subject, body, style_db).await?)
}

pub async fn add_tag(
    db: &PgPool,
    author: Uuid,
    note_id: Uuid,
    kind: NoteTagKind,
    tag: &str,
) -> Result<PlayerNoteTagRow, GqlError> {
    ensure_owned(db, author, note_id).await?;
    let tag = tag.trim();
    if tag.is_empty() {
        return Err(GqlError::new("Tag cannot be empty"));
    }
    Ok(player_notes::add_tag(db, note_id, kind.as_db(), tag).await?)
}

pub async fn remove_tag(
    db: &PgPool,
    author: Uuid,
    note_id: Uuid,
    tag_id: Uuid,
) -> Result<bool, GqlError> {
    ensure_owned(db, author, note_id).await?;
    Ok(player_notes::remove_tag(db, note_id, tag_id).await?)
}

pub async fn add_showdown(
    db: &PgPool,
    author: Uuid,
    note_id: Uuid,
    tournament_id: Option<Uuid>,
    description: &str,
) -> Result<ShowdownObservationRow, GqlError> {
    ensure_owned(db, author, note_id).await?;
    let description = description.trim();
    if description.is_empty() {
        return Err(GqlError::new("Description cannot be empty"));
    }
    Ok(player_notes::add_showdown(db, note_id, tournament_id, description).await?)
}

/// Author-only invariant for child mutations: the note must belong to the author.
async fn ensure_owned(db: &PgPool, author: Uuid, note_id: Uuid) -> Result<(), GqlError> {
    player_notes::get_owned(db, author, note_id)
        .await?
        .ok_or_else(|| GqlError::new("Note not found"))?;
    Ok(())
}
