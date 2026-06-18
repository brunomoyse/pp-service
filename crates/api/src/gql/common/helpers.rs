use async_graphql::Context;
use uuid::Uuid;

use crate::auth::permissions::viewer_manages_club;
use crate::state::AppState;

pub async fn get_club_id_for_tournament(
    db: &infra::db::Db,
    tournament_id: Uuid,
) -> async_graphql::Result<Uuid> {
    let tournament = infra::repos::tournaments::get_by_id(db, tournament_id)
        .await?
        .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;
    Ok(tournament.club_id)
}

/// Whether a tournament must be hidden from the current viewer because it
/// belongs to a free ("Home Game") club. Free clubs are private host tools:
/// invisible to the player app and public, but still fully visible to their own
/// managers and to admins. Non-existent tournaments resolve to `false` so the
/// caller's normal not-found handling applies.
pub async fn tournament_hidden_from_viewer(
    ctx: &Context<'_>,
    tournament_id: Uuid,
) -> async_graphql::Result<bool> {
    let state = ctx.data::<AppState>()?;
    let row: Option<(String, Uuid)> = sqlx::query_as(
        "SELECT c.plan, c.id FROM tournaments t JOIN clubs c ON c.id = t.club_id WHERE t.id = $1",
    )
    .bind(tournament_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| async_graphql::Error::new("Database operation failed"))?;

    match row {
        Some((plan, club_id)) if plan == "free" => Ok(!viewer_manages_club(ctx, club_id).await),
        _ => Ok(false),
    }
}
