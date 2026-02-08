use async_graphql::{Enum, SimpleObject};

use crate::gql::types::User;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum LeaderboardPeriod {
    AllTime,
    LastYear,
    Last6Months,
    Last30Days,
    Last7Days,
}

impl From<LeaderboardPeriod> for infra::repos::tournament_results::LeaderboardPeriod {
    fn from(period: LeaderboardPeriod) -> Self {
        match period {
            LeaderboardPeriod::AllTime => {
                infra::repos::tournament_results::LeaderboardPeriod::AllTime
            }
            LeaderboardPeriod::LastYear => {
                infra::repos::tournament_results::LeaderboardPeriod::LastYear
            }
            LeaderboardPeriod::Last6Months => {
                infra::repos::tournament_results::LeaderboardPeriod::Last6Months
            }
            LeaderboardPeriod::Last30Days => {
                infra::repos::tournament_results::LeaderboardPeriod::Last30Days
            }
            LeaderboardPeriod::Last7Days => {
                infra::repos::tournament_results::LeaderboardPeriod::Last7Days
            }
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct LeaderboardEntry {
    pub user: User, // Full user object with complete info
    pub rank: i32,  // Position in leaderboard (1-based)
    pub total_tournaments: i32,
    pub total_buy_ins: i32,  // Total amount spent (cents)
    pub total_winnings: i32, // Total amount won (cents)
    pub net_profit: i32,     // winnings - buy_ins (cents)
    pub total_itm: i32,      // Number of tournaments where player finished in the money
    pub itm_percentage: f64, // (total_itm / total_tournaments) * 100
    pub roi_percentage: f64, // ((total_winnings - total_buy_ins) / total_buy_ins) * 100
    pub average_finish: f64, // Average finishing position
    pub first_places: i32,   // Number of first place finishes
    pub final_tables: i32,   // Number of final table finishes (top 9)
    pub points: f64,         // Calculated leaderboard points
}

#[derive(SimpleObject)]
pub struct LeaderboardResponse {
    pub entries: Vec<LeaderboardEntry>,
    pub total_players: i32,
    pub period: LeaderboardPeriod,
}
