use async_graphql::{Enum, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};

use crate::gql::types::Tournament;

#[derive(SimpleObject, Clone)]
pub struct TournamentResult {
    pub id: ID,
    pub tournament_id: ID,
    pub user_id: ID,
    pub final_position: i32,
    pub prize_cents: i32,
    pub points: i32,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(SimpleObject, Clone)]
pub struct UserTournamentResult {
    pub result: TournamentResult,
    pub tournament: Tournament,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum DealType {
    EvenSplit,
    Icm,
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
pub struct PayoutPosition {
    pub position: i32,
    pub percentage: f64,
    pub amount_cents: i32,
}

#[derive(SimpleObject, Clone)]
pub struct TournamentPayout {
    pub id: ID,
    pub tournament_id: ID,
    pub template_id: Option<ID>,
    pub player_count: i32,
    pub total_prize_pool: i32,
    pub positions: Vec<PayoutPosition>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
