//! Private opponent notes.
//!
//! AUTHOR-ONLY INVARIANT: notes are visible only to their author. Every function
//! that loads a note (or its children) is scoped by `author_app_user_id`, either
//! directly or by first resolving the parent note through an author-scoped query.
//! There is deliberately no "get note by id" that ignores the author.

use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::{PlayerNoteRow, PlayerNoteTagRow, ShowdownObservationRow};

/// One player in a tournament's field, with the viewer's note flattened in.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FieldNoteRow {
    pub rp_id: Uuid,
    pub rp_club_id: Uuid,
    pub rp_display_name: String,
    pub rp_app_user_id: Option<Uuid>,
    pub rp_created_at: DateTime<Utc>,
    pub rp_updated_at: DateTime<Utc>,
    pub pn_id: Option<Uuid>,
    pub pn_body: Option<String>,
    pub pn_style: Option<String>,
    pub pn_created_at: Option<DateTime<Utc>>,
    pub pn_updated_at: Option<DateTime<Utc>>,
}

const NOTE_COLS: &str =
    "id, author_app_user_id, subject_club_player_id, body, style, created_at, updated_at";

/// The author's note on a specific subject, if one exists.
pub async fn get_for_subject<'e>(
    executor: impl PgExecutor<'e>,
    author_app_user_id: Uuid,
    subject_club_player_id: Uuid,
) -> SqlxResult<Option<PlayerNoteRow>> {
    sqlx::query_as::<_, PlayerNoteRow>(&format!(
        "SELECT {NOTE_COLS} FROM player_note \
         WHERE author_app_user_id = $1 AND subject_club_player_id = $2"
    ))
    .bind(author_app_user_id)
    .bind(subject_club_player_id)
    .fetch_optional(executor)
    .await
}

/// A note by id, but only if it belongs to the given author (else None).
pub async fn get_owned<'e>(
    executor: impl PgExecutor<'e>,
    author_app_user_id: Uuid,
    note_id: Uuid,
) -> SqlxResult<Option<PlayerNoteRow>> {
    sqlx::query_as::<_, PlayerNoteRow>(&format!(
        "SELECT {NOTE_COLS} FROM player_note WHERE id = $1 AND author_app_user_id = $2"
    ))
    .bind(note_id)
    .bind(author_app_user_id)
    .fetch_optional(executor)
    .await
}

/// All of the author's notes, newest first.
pub async fn list_for_author<'e>(
    executor: impl PgExecutor<'e>,
    author_app_user_id: Uuid,
) -> SqlxResult<Vec<PlayerNoteRow>> {
    sqlx::query_as::<_, PlayerNoteRow>(&format!(
        "SELECT {NOTE_COLS} FROM player_note WHERE author_app_user_id = $1 ORDER BY updated_at DESC"
    ))
    .bind(author_app_user_id)
    .fetch_all(executor)
    .await
}

/// How many distinct subjects the author has notes on (for the free-tier cap).
pub async fn count_subjects_for_author<'e>(
    executor: impl PgExecutor<'e>,
    author_app_user_id: Uuid,
) -> SqlxResult<i64> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM player_note WHERE author_app_user_id = $1")
        .bind(author_app_user_id)
        .fetch_one(executor)
        .await
}

/// Create or update the author's note on a subject (body + style).
pub async fn upsert<'e>(
    executor: impl PgExecutor<'e>,
    author_app_user_id: Uuid,
    subject_club_player_id: Uuid,
    body: &str,
    style: Option<&str>,
) -> SqlxResult<PlayerNoteRow> {
    sqlx::query_as::<_, PlayerNoteRow>(&format!(
        "INSERT INTO player_note (author_app_user_id, subject_club_player_id, body, style) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (author_app_user_id, subject_club_player_id) DO UPDATE SET \
            body = EXCLUDED.body, style = EXCLUDED.style, updated_at = NOW() \
         RETURNING {NOTE_COLS}"
    ))
    .bind(author_app_user_id)
    .bind(subject_club_player_id)
    .bind(body)
    .bind(style)
    .fetch_one(executor)
    .await
}

/// Delete the author's note on a subject. Returns true if a row was removed.
pub async fn delete_owned<'e>(
    executor: impl PgExecutor<'e>,
    author_app_user_id: Uuid,
    note_id: Uuid,
) -> SqlxResult<bool> {
    let affected = sqlx::query("DELETE FROM player_note WHERE id = $1 AND author_app_user_id = $2")
        .bind(note_id)
        .bind(author_app_user_id)
        .execute(executor)
        .await?
        .rows_affected();
    Ok(affected > 0)
}

// --- tags / tells (parent note must already be author-verified) ---

pub async fn list_tags<'e>(
    executor: impl PgExecutor<'e>,
    note_id: Uuid,
) -> SqlxResult<Vec<PlayerNoteTagRow>> {
    sqlx::query_as::<_, PlayerNoteTagRow>(
        "SELECT id, note_id, kind, tag, created_at FROM player_note_tag \
         WHERE note_id = $1 ORDER BY created_at ASC",
    )
    .bind(note_id)
    .fetch_all(executor)
    .await
}

pub async fn add_tag<'e>(
    executor: impl PgExecutor<'e>,
    note_id: Uuid,
    kind: &str,
    tag: &str,
) -> SqlxResult<PlayerNoteTagRow> {
    sqlx::query_as::<_, PlayerNoteTagRow>(
        "INSERT INTO player_note_tag (note_id, kind, tag) VALUES ($1, $2, $3) \
         ON CONFLICT (note_id, kind, tag) DO UPDATE SET tag = EXCLUDED.tag \
         RETURNING id, note_id, kind, tag, created_at",
    )
    .bind(note_id)
    .bind(kind)
    .bind(tag)
    .fetch_one(executor)
    .await
}

/// Remove a tag, scoped to a note the caller owns (note_id is author-verified upstream).
pub async fn remove_tag<'e>(
    executor: impl PgExecutor<'e>,
    note_id: Uuid,
    tag_id: Uuid,
) -> SqlxResult<bool> {
    let affected = sqlx::query("DELETE FROM player_note_tag WHERE id = $1 AND note_id = $2")
        .bind(tag_id)
        .bind(note_id)
        .execute(executor)
        .await?
        .rows_affected();
    Ok(affected > 0)
}

// --- showdown observations (parent note must already be author-verified) ---

pub async fn list_showdowns<'e>(
    executor: impl PgExecutor<'e>,
    note_id: Uuid,
) -> SqlxResult<Vec<ShowdownObservationRow>> {
    sqlx::query_as::<_, ShowdownObservationRow>(
        "SELECT id, note_id, tournament_id, description, created_at FROM showdown_observation \
         WHERE note_id = $1 ORDER BY created_at DESC",
    )
    .bind(note_id)
    .fetch_all(executor)
    .await
}

pub async fn add_showdown<'e>(
    executor: impl PgExecutor<'e>,
    note_id: Uuid,
    tournament_id: Option<Uuid>,
    description: &str,
) -> SqlxResult<ShowdownObservationRow> {
    sqlx::query_as::<_, ShowdownObservationRow>(
        "INSERT INTO showdown_observation (note_id, tournament_id, description) \
         VALUES ($1, $2, $3) RETURNING id, note_id, tournament_id, description, created_at",
    )
    .bind(note_id)
    .bind(tournament_id)
    .bind(description)
    .fetch_one(executor)
    .await
}

/// Everyone registered for a tournament, each with the viewer's private note (if
/// any). Author-scoped: only `$2`'s own notes are joined, and the viewer's own
/// roster entry is excluded. Players the viewer has a note on are listed first.
pub async fn field_with_notes<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    author_app_user_id: Uuid,
) -> SqlxResult<Vec<FieldNoteRow>> {
    sqlx::query_as::<_, FieldNoteRow>(
        "SELECT \
            rp.id AS rp_id, rp.club_id AS rp_club_id, rp.display_name AS rp_display_name, \
            rp.app_user_id AS rp_app_user_id, rp.created_at AS rp_created_at, \
            rp.updated_at AS rp_updated_at, \
            pn.id AS pn_id, pn.body AS pn_body, pn.style AS pn_style, \
            pn.created_at AS pn_created_at, pn.updated_at AS pn_updated_at \
         FROM tournament_registrations reg \
         JOIN club_player rp ON rp.id = reg.club_player_id \
         LEFT JOIN player_note pn \
            ON pn.subject_club_player_id = rp.id AND pn.author_app_user_id = $2 \
         WHERE reg.tournament_id = $1 \
           AND reg.status NOT IN ('cancelled', 'no_show') \
           AND rp.app_user_id IS DISTINCT FROM $2 \
         ORDER BY (pn.id IS NOT NULL) DESC, rp.display_name ASC",
    )
    .bind(tournament_id)
    .bind(author_app_user_id)
    .fetch_all(executor)
    .await
}
