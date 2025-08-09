use async_graphql::{SimpleObject, ID};

#[derive(SimpleObject, Clone)]
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