use async_graphql::{Context, Object, Result};
use infra::repos::tournament_results;

use crate::gql::types::{PaginatedResponse, PaginationInput, Role, User};
use crate::state::AppState;

use super::types::{LeaderboardEntry, LeaderboardPeriod};

#[derive(Default)]
pub struct LeaderboardQuery;

#[Object]
impl LeaderboardQuery {
    /// Get player leaderboard with comprehensive statistics and points
    async fn leaderboard(
        &self,
        ctx: &Context<'_>,
        period: Option<LeaderboardPeriod>,
        pagination: Option<PaginationInput>,
        club_id: Option<uuid::Uuid>,
        #[graphql(
            desc = "Province slug (see clubProvinces); ranks players across every club in that province."
        )]
        province: Option<String>,
    ) -> Result<PaginatedResponse<LeaderboardEntry>> {
        let state = ctx.data::<AppState>()?;

        let period = period.unwrap_or(LeaderboardPeriod::AllTime);
        let infra_period: infra::repos::tournament_results::LeaderboardPeriod = period.into();

        let page_params = pagination.unwrap_or(PaginationInput {
            limit: Some(100),
            offset: Some(0),
        });
        let limit_offset = page_params.to_limit_offset();

        // Fetch leaderboard and total count in parallel
        let (leaderboard_entries, total_count) = tokio::try_join!(
            tournament_results::get_leaderboard(
                &state.db,
                infra_period,
                Some(limit_offset.limit as i32),
                Some(limit_offset.offset as i32),
                club_id,
                province.clone(),
            ),
            tournament_results::count_leaderboard(
                &state.db,
                infra_period,
                club_id,
                province.clone()
            )
        )?;

        // Convert to GraphQL types and add rank based on offset
        let offset = limit_offset.offset as i32;
        let entries: Vec<LeaderboardEntry> = leaderboard_entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| LeaderboardEntry {
                registered_player_id: entry.registered_player_id.into(),
                display_name: entry.display_name.clone(),
                // Attach the app user only when this roster player has an account.
                user: entry.user_id.map(|uid| User {
                    id: uid.into(),
                    email: entry.email.clone().unwrap_or_default(),
                    username: entry.username.clone(),
                    first_name: entry.first_name.clone().unwrap_or_default(),
                    last_name: entry.last_name.clone(),
                    phone: entry.phone.clone(),
                    is_active: entry.is_active.unwrap_or(true),
                    role: Role::from(entry.role.clone()),
                    locale: entry.locale.clone().unwrap_or_default(),
                }),
                rank: offset + (index as i32) + 1, // 1-based ranking with offset
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

        let page_size = entries.len() as i32;
        let has_next_page = (offset + page_size) < total_count as i32;

        Ok(PaginatedResponse {
            items: entries,
            total_count: total_count as i32,
            page_size,
            offset,
            has_next_page,
        })
    }
}
