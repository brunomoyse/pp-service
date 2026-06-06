use async_graphql::{SimpleObject, ID};
use chrono::{DateTime, Utc};

/// The current user's Prediction-Points standing. PP is a free, earned-only
/// fantasy currency — never bought, never cashed out (G2).
#[derive(SimpleObject, Clone, Debug)]
pub struct PredictionBalance {
    pub balance: i32,
    /// Points the user can claim right now from attendance/play (+ first-time seed).
    pub claimable: i32,
}

/// A fantasy pick on a tournament winner.
#[derive(SimpleObject, Clone, Debug)]
pub struct PredictionEntry {
    pub id: ID,
    pub tournament_id: ID,
    pub tournament_name: String,
    pub predicted_winner_name: String,
    pub stake_points: i32,
    /// `open`, `won`, or `lost`.
    pub status: String,
    pub payout_points: i32,
    pub created_at: DateTime<Utc>,
}

impl From<infra::models::PredictionEntryView> for PredictionEntry {
    fn from(v: infra::models::PredictionEntryView) -> Self {
        Self {
            id: v.id.into(),
            tournament_id: v.tournament_id.into(),
            tournament_name: v.tournament_name,
            predicted_winner_name: v.predicted_winner_name,
            stake_points: v.stake_points,
            status: v.status,
            payout_points: v.payout_points,
            created_at: v.created_at,
        }
    }
}
