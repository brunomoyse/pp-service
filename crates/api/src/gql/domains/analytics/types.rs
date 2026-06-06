use async_graphql::{SimpleObject, ID};

use infra::repos::analytics as repo;

fn clamp_i32(v: i64) -> i32 {
    v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

#[derive(SimpleObject, Clone, Debug)]
pub struct ClubBreakdown {
    pub club_id: ID,
    pub club_name: String,
    pub tournaments: i32,
    pub buyins_cents: i32,
    pub winnings_cents: i32,
    pub net_cents: i32,
}

impl From<repo::ClubBreakdownRow> for ClubBreakdown {
    fn from(r: repo::ClubBreakdownRow) -> Self {
        Self {
            club_id: r.club_id.into(),
            club_name: r.club_name,
            tournaments: clamp_i32(r.tournaments),
            buyins_cents: clamp_i32(r.buyins_cents),
            winnings_cents: clamp_i32(r.winnings_cents),
            net_cents: clamp_i32(r.winnings_cents - r.buyins_cents),
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct BuyInBreakdown {
    pub buy_in_cents: i32,
    pub tournaments: i32,
    pub buyins_cents: i32,
    pub winnings_cents: i32,
    pub net_cents: i32,
}

impl From<repo::BuyInBreakdownRow> for BuyInBreakdown {
    fn from(r: repo::BuyInBreakdownRow) -> Self {
        Self {
            buy_in_cents: r.buy_in_cents,
            tournaments: clamp_i32(r.tournaments),
            buyins_cents: clamp_i32(r.buyins_cents),
            winnings_cents: clamp_i32(r.winnings_cents),
            net_cents: clamp_i32(r.winnings_cents - r.buyins_cents),
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct PnlPoint {
    /// ISO date (YYYY-MM-DD) of the play day.
    pub day: String,
    pub net_cents: i32,
    pub cumulative_cents: i32,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct ProAnalytics {
    pub by_club: Vec<ClubBreakdown>,
    pub by_buy_in: Vec<BuyInBreakdown>,
    pub cumulative_pnl: Vec<PnlPoint>,
}
