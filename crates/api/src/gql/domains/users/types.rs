use async_graphql::{InputObject, ID};

#[derive(InputObject)]
pub struct CreatePlayerInput {
    pub email: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub phone: Option<String>,
    pub club_id: ID,
}

#[derive(InputObject)]
pub struct UpdatePlayerInput {
    pub id: ID,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub phone: Option<String>,
}
