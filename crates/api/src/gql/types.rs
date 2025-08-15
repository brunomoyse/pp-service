use async_graphql::{Context, Error, ComplexObject, Result, SimpleObject, InputObject, ID};
use async_graphql::dataloader::DataLoader;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::gql::loaders::ClubLoader;

#[derive(SimpleObject, Clone)]
#[graphql(complex)]
pub struct Tournament {
    pub id: ID,
    pub title: String,
    pub club_id: ID,
}

#[derive(SimpleObject, Clone)]
pub struct Club {
    pub id: ID,
    pub name: String,
    pub city: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct User {
    pub id: ID,
    pub email: String,
    pub username: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentRegistration {
    pub id: ID,
    pub tournament_id: ID,
    pub user_id: ID,
    pub registration_time: DateTime<Utc>,
    pub status: String,
    pub notes: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentPlayer {
    pub registration: TournamentRegistration,
    pub user: User,
}

#[derive(SimpleObject, Clone)]
pub struct PlayerRegistrationEvent {
    pub tournament_id: ID,
    pub player: TournamentPlayer,
    pub event_type: String,
}

#[derive(InputObject)]
pub struct RegisterForTournamentInput {
    pub tournament_id: ID,
    pub notes: Option<String>,
}

#[ComplexObject]
impl Tournament {
    async fn club(&self, ctx: &Context<'_>) -> Result<Club> {
        let loader = ctx.data::<DataLoader<ClubLoader>>()?;
        let club_uuid =
            Uuid::parse_str(self.club_id.as_str()).map_err(|e| Error::new(e.to_string()))?;

        match loader
            .load_one(club_uuid)
            .await
            .map_err(|e| Error::new(e.to_string()))?
        {
            Some(row) => Ok(Club { id: row.id.into(), name: row.name, city: row.city }),
            None => Err(Error::new("Club not found")),
        }
    }
}