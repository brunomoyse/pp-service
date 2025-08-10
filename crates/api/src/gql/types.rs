use async_graphql::{Context, Error, ComplexObject, Result, SimpleObject, ID};
use async_graphql::dataloader::DataLoader;
use uuid::Uuid;

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