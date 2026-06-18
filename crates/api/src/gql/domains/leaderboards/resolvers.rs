use async_graphql::{Context, Object, Result};
use infra::repos::tournament_results;

use crate::auth::permissions::{viewer_is_admin, viewer_manages_club};
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
        #[graphql(
            desc = "League id (see leaderboardConfigs). When set, points are recomputed from the league's formula and `period` is ignored (the league's own period applies)."
        )]
        config_id: Option<uuid::Uuid>,
    ) -> Result<PaginatedResponse<LeaderboardEntry>> {
        let state = ctx.data::<AppState>()?;

        let period = period.unwrap_or(LeaderboardPeriod::AllTime);
        let infra_period: infra::repos::tournament_results::LeaderboardPeriod = period.into();

        let page_params = pagination.unwrap_or(PaginationInput {
            limit: Some(100),
            offset: Some(0),
        });
        let limit_offset = page_params.to_limit_offset();

        // League path: recompute points on read from the league's formula.
        if let Some(config_uuid) = config_id {
            return league_leaderboard(ctx, state, config_uuid, &limit_offset).await;
        }

        // Free ("Home Game") clubs never appear in player-facing leaderboards.
        // A club-scoped request from that club's own manager (or an admin) is
        // the only way to include them; global/province scope excludes them
        // unless the viewer is admin.
        let exclude_free = match club_id {
            Some(cid) => !viewer_manages_club(ctx, cid).await,
            None => !viewer_is_admin(ctx),
        };

        // Fetch leaderboard and total count in parallel
        let (leaderboard_entries, total_count) = tokio::try_join!(
            tournament_results::get_leaderboard(
                &state.db,
                infra_period,
                Some(limit_offset.limit as i32),
                Some(limit_offset.offset as i32),
                club_id,
                province.clone(),
                exclude_free,
            ),
            tournament_results::count_leaderboard(
                &state.db,
                infra_period,
                club_id,
                province.clone(),
                exclude_free,
            )
        )?;

        // Convert to GraphQL types and add rank based on offset
        let offset = limit_offset.offset as i32;
        let entries: Vec<LeaderboardEntry> = leaderboard_entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| to_gql_entry(entry, offset + (index as i32) + 1))
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

/// Map an infra leaderboard row to the GraphQL type, stamping its 1-based rank.
fn to_gql_entry(
    entry: infra::repos::tournament_results::LeaderboardEntry,
    rank: i32,
) -> LeaderboardEntry {
    LeaderboardEntry {
        club_player_id: entry.club_player_id.into(),
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
        rank,
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
    }
}

/// League leaderboard: recompute points on read from the league's formula.
async fn league_leaderboard(
    ctx: &Context<'_>,
    state: &AppState,
    config_id: uuid::Uuid,
    limit_offset: &infra::pagination::LimitOffset,
) -> Result<PaginatedResponse<LeaderboardEntry>> {
    let config = infra::repos::leaderboard_configs::get_by_id(&state.db, config_id)
        .await?
        .ok_or_else(|| async_graphql::Error::new("League not found"))?;
    let formula: infra::scoring::ScoringFormula =
        serde_json::from_value(config.formula_params).unwrap_or_default();

    // A free club's league is hidden from the player app; only its own managers
    // and admins can read it.
    let exclude_free = !viewer_manages_club(ctx, config.club_id).await;

    let (rows, total_count) = tournament_results::get_leaderboard_for_config(
        &state.db,
        config.id,
        &formula,
        &config.membership_mode,
        config.club_id,
        config.period_start,
        config.period_end,
        Some(limit_offset.limit as i32),
        Some(limit_offset.offset as i32),
        exclude_free,
    )
    .await?;

    let offset = limit_offset.offset as i32;
    let entries: Vec<LeaderboardEntry> = rows
        .into_iter()
        .enumerate()
        .map(|(index, entry)| to_gql_entry(entry, offset + (index as i32) + 1))
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
