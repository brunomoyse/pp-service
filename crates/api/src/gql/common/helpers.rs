use uuid::Uuid;

pub async fn get_club_id_for_tournament(
    db: &infra::db::Db,
    tournament_id: Uuid,
) -> async_graphql::Result<Uuid> {
    let tournament = infra::repos::tournaments::get_by_id(db, tournament_id)
        .await?
        .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;
    Ok(tournament.club_id)
}
