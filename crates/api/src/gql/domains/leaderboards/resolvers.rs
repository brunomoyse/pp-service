use async_graphql::{Context, Object, Result};
use infra::repos::tournament_results;

use crate::gql::types::{Role, User};
use crate::state::AppState;

use super::types::{LeaderboardEntry, LeaderboardPeriod, LeaderboardResponse};

#[derive(Default)]
pub struct LeaderboardQuery;

#[Object]
impl LeaderboardQuery {
    /// Get player leaderboard with comprehensive statistics and points
    async fn leaderboard(
        &self,
        ctx: &Context<'_>,
        period: Option<LeaderboardPeriod>,
        limit: Option<i32>,
        club_id: Option<uuid::Uuid>,
    ) -> Result<LeaderboardResponse> {
        let state = ctx.data::<AppState>()?;

        let period = period.unwrap_or(LeaderboardPeriod::AllTime);
        let infra_period: infra::repos::tournament_results::LeaderboardPeriod = period.into();

        let leaderboard_entries =
            tournament_results::get_leaderboard(&state.db, infra_period, limit, club_id).await?;

        // Convert to GraphQL types and add rank
        let entries: Vec<LeaderboardEntry> = leaderboard_entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| LeaderboardEntry {
                user: User {
                    id: entry.user_id.into(),
                    email: entry.email,
                    username: entry.username,
                    first_name: entry.first_name,
                    last_name: entry.last_name,
                    phone: entry.phone,
                    is_active: entry.is_active,
                    role: Role::from(entry.role),
                },
                rank: (index + 1) as i32, // 1-based ranking
                total_tournaments: entry.total_tournaments,
                total_buy_ins: entry.total_buy_ins,
                total_winnings: entry.total_winnings,
                net_profit: entry.net_profit,
                total_itm: entry.total_itm,
                itm_percentage: entry.itm_percentage,
                roi_percentage: entry.roi_percentage,
                average_finish: entry.average_finish,
                first_places: entry.first_places,
                final_tables: entry.final_tables,
                points: entry.points,
            })
            .collect();

        let total_players = entries.len() as i32;

        Ok(LeaderboardResponse {
            entries,
            total_players,
            period,
        })
    }
}
