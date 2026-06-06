use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::auth::permissions::require_pro;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::analytics;

use super::types::{BuyInBreakdown, ClubBreakdown, PnlPoint, ProAnalytics};

#[derive(Default)]
pub struct AnalyticsQuery;

#[Object]
impl AnalyticsQuery {
    /// Personal performance analytics — breakdowns and cumulative P/L. Pro only.
    async fn my_pro_analytics(&self, ctx: &Context<'_>) -> Result<ProAnalytics> {
        let user = require_pro(ctx).await?;
        let user_id = Uuid::parse_str(user.id.as_str()).gql_err("Invalid user ID")?;
        let state = ctx.data::<AppState>()?;

        let by_club = analytics::by_club(&state.db, user_id)
            .await?
            .into_iter()
            .map(ClubBreakdown::from)
            .collect();

        let by_buy_in = analytics::by_buy_in(&state.db, user_id)
            .await?
            .into_iter()
            .map(BuyInBreakdown::from)
            .collect();

        // Fold the per-day net into a running cumulative for the P/L graph.
        let mut cumulative: i64 = 0;
        let cumulative_pnl = analytics::pnl_timeline(&state.db, user_id)
            .await?
            .into_iter()
            .map(|p| {
                cumulative += p.net_cents;
                PnlPoint {
                    day: p.day.to_string(),
                    net_cents: p.net_cents.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                    cumulative_cents: cumulative.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                }
            })
            .collect();

        Ok(ProAnalytics {
            by_club,
            by_buy_in,
            cumulative_pnl,
        })
    }
}
