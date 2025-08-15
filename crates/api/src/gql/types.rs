use async_graphql::{Context, Error, ComplexObject, Result, SimpleObject, InputObject, ID, Enum};
use async_graphql::dataloader::DataLoader;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::gql::loaders::ClubLoader;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Role {
    #[graphql(name = "MANAGER")]
    Manager,
    #[graphql(name = "PLAYER")]
    Player,
}

impl From<String> for Role {
    fn from(role: String) -> Self {
        match role.as_str() {
            "manager" => Role::Manager,
            "player" => Role::Player,
            _ => Role::Player, // Default to player for invalid roles
        }
    }
}

impl From<Option<String>> for Role {
    fn from(role: Option<String>) -> Self {
        match role {
            Some(r) => Role::from(r),
            None => Role::Player, // Default to player if no role specified
        }
    }
}

impl From<Role> for String {
    fn from(role: Role) -> Self {
        match role {
            Role::Manager => "manager".to_string(),
            Role::Player => "player".to_string(),
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum TournamentStatus {
    #[graphql(name = "UPCOMING")]
    Upcoming,
    #[graphql(name = "ONGOING")]
    Ongoing,
    #[graphql(name = "ENDED")]
    Ended,
}

impl From<TournamentStatus> for infra::repos::tournaments::TournamentStatus {
    fn from(status: TournamentStatus) -> Self {
        match status {
            TournamentStatus::Upcoming => infra::repos::tournaments::TournamentStatus::Upcoming,
            TournamentStatus::Ongoing => infra::repos::tournaments::TournamentStatus::Ongoing,
            TournamentStatus::Ended => infra::repos::tournaments::TournamentStatus::Ended,
        }
    }
}

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

#[derive(SimpleObject, Clone, serde::Serialize)]
pub struct User {
    pub id: ID,
    pub email: String,
    pub username: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
    pub role: Role,
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
pub struct TournamentResult {
    pub id: ID,
    pub tournament_id: ID,
    pub user_id: ID,
    pub final_position: i32,
    pub prize_cents: i32,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(SimpleObject, Clone)]
pub struct UserTournamentResult {
    pub result: TournamentResult,
    pub tournament: Tournament,
}

#[derive(SimpleObject, Clone)]
pub struct PayoutPosition {
    pub position: i32,
    pub percentage: f64,
}

#[derive(SimpleObject, Clone)]
pub struct PayoutTemplate {
    pub id: ID,
    pub name: String,
    pub description: Option<String>,
    pub min_players: i32,
    pub max_players: Option<i32>,
    pub payout_structure: Vec<PayoutPosition>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum DealType {
    #[graphql(name = "EVEN_SPLIT")]
    EvenSplit,
    #[graphql(name = "ICM")]
    Icm,
    #[graphql(name = "CUSTOM")]
    Custom,
}

#[derive(SimpleObject, Clone)]
pub struct CustomPayout {
    pub user_id: ID,
    pub amount_cents: i32,
}

#[derive(SimpleObject, Clone)]
pub struct PlayerDeal {
    pub id: ID,
    pub tournament_id: ID,
    pub deal_type: DealType,
    pub affected_positions: Vec<i32>,
    pub custom_payouts: Option<Vec<CustomPayout>>,
    pub total_amount_cents: i32,
    pub notes: Option<String>,
    pub created_by: ID,
}

#[derive(InputObject)]
pub struct EnterTournamentResultsInput {
    pub tournament_id: ID,
    pub payout_template_id: Option<ID>,
    pub player_positions: Vec<PlayerPositionInput>,
    pub deal: Option<PlayerDealInput>,
}

#[derive(InputObject)]
pub struct PlayerPositionInput {
    pub user_id: ID,
    pub final_position: i32,
}

#[derive(InputObject)]
pub struct PlayerDealInput {
    pub deal_type: DealType,
    pub affected_positions: Vec<i32>,
    pub custom_payouts: Option<Vec<CustomPayoutInput>>,
    pub notes: Option<String>,
}

#[derive(InputObject)]
pub struct CustomPayoutInput {
    pub user_id: ID,
    pub amount_cents: i32,
}

#[derive(SimpleObject)]
pub struct EnterTournamentResultsResponse {
    pub success: bool,
    pub results: Vec<TournamentResult>,
    pub deal: Option<PlayerDeal>,
}

#[derive(SimpleObject, Clone)]
pub struct PlayerStatistics {
    pub total_itm: i32,
    pub total_tournaments: i32,
    pub total_winnings: i32,
    pub total_buy_ins: i32,
    pub itm_percentage: f64,
    pub roi_percentage: f64,
}

#[derive(SimpleObject)]
pub struct PlayerStatsResponse {
    pub last_7_days: PlayerStatistics,
    pub last_30_days: PlayerStatistics,
    pub last_year: PlayerStatistics,
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
    pub user_id: Option<ID>, // Optional: if provided, admin can register another user
    pub notes: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct AuthPayload {
    pub token: String,
    pub user: User,
}

#[derive(SimpleObject, Clone)]
pub struct OAuthUrlResponse {
    pub auth_url: String,
    pub csrf_token: String,
}

#[derive(InputObject)]
pub struct OAuthCallbackInput {
    pub provider: String,
    pub code: String,
    pub csrf_token: String,
}

#[derive(SimpleObject, Clone)]
pub struct OAuthClient {
    pub id: ID,
    pub client_id: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub is_active: bool,
}

#[derive(InputObject)]
pub struct CreateOAuthClientInput {
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Option<Vec<String>>,
}

#[derive(SimpleObject)]
pub struct CreateOAuthClientResponse {
    pub client: OAuthClient,
    pub client_secret: String,
}

#[derive(InputObject)]
pub struct UserRegistrationInput {
    pub email: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub username: Option<String>,
}

#[derive(InputObject)]
pub struct UserLoginInput {
    pub email: String,
    pub password: String,
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