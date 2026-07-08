use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::{privacy, scouting};

use super::service;
use super::types::{PrivacySettings, ScoutingMatch, ScoutingProfile, ScoutingQuota};

/// Minimum query length so lookup can't be used to enumerate the whole pool.
const MIN_QUERY_LEN: usize = 2;
const SEARCH_LIMIT: i64 = 20;

fn current_user_id(ctx: &Context<'_>) -> Result<Uuid> {
    let claims = ctx.data::<Claims>()?;
    Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")
}

#[derive(Default)]
pub struct ScoutingQuery;

#[Object]
impl ScoutingQuery {
    /// The current user's privacy/consent settings (defaults all OFF).
    async fn my_privacy_settings(&self, ctx: &Context<'_>) -> Result<PrivacySettings> {
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let row = privacy::get(&state.db, user_id).await?;
        Ok(row.map(PrivacySettings::from).unwrap_or_default())
    }

    /// Search the scouting pool by handle. Only opted-in players are returned;
    /// this is free (no quota) and returns handles only — no stats.
    async fn scouting_search(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> Result<Vec<ScoutingMatch>> {
        current_user_id(ctx)?; // auth required
        let trimmed = query.trim();
        if trimmed.len() < MIN_QUERY_LEN {
            return Ok(vec![]);
        }
        let state = ctx.data::<AppState>()?;
        let rows = scouting::search_pool(&state.db, trimmed, SEARCH_LIMIT).await?;
        Ok(rows.into_iter().map(ScoutingMatch::from).collect())
    }

    /// View a pool member's full scouting profile. Lookups are unlimited.
    async fn scouting_profile(&self, ctx: &Context<'_>, user_id: ID) -> Result<ScoutingProfile> {
        let state = ctx.data::<AppState>()?;
        let searcher = current_user_id(ctx)?;
        let target = Uuid::parse_str(user_id.as_str()).gql_err("Invalid user ID")?;

        let r = service::lookup(&state.db, searcher, target).await?;
        let tournaments = r.stats.tournaments;
        let itm_percentage = if tournaments > 0 {
            (r.stats.itm_count as f64 / tournaments as f64) * 100.0
        } else {
            0.0
        };
        Ok(ScoutingProfile {
            user_id,
            handle: r.handle,
            tournaments: tournaments as i32,
            itm_percentage,
            best_finish: r.stats.best_finish,
            shares_pnl: r.shares_pnl,
            net_cents: if r.shares_pnl {
                Some(r.stats.net_cents as i32)
            } else {
                None
            },
        })
    }

    /// The current user's scouting lookup status (unlimited for all).
    async fn my_scouting_quota(&self, ctx: &Context<'_>) -> Result<ScoutingQuota> {
        let state = ctx.data::<AppState>()?;
        let searcher = current_user_id(ctx)?;
        let q = service::quota_status(&state.db, searcher).await?;
        Ok(ScoutingQuota {
            used: q.used as i32,
            limit: q.limit as i32,
            unlimited: q.unlimited,
        })
    }
}

#[derive(Default)]
pub struct ScoutingMutation;

#[Object]
impl ScoutingMutation {
    /// Update the current user's consent flags. Both are granular and explicit
    /// (G4): the client must send each value, there is no implied bundling.
    async fn update_privacy_settings(
        &self,
        ctx: &Context<'_>,
        share_named_pl: bool,
        in_scouting_pool: bool,
    ) -> Result<PrivacySettings> {
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let row = privacy::upsert(&state.db, user_id, share_named_pl, in_scouting_pool).await?;
        Ok(PrivacySettings::from(row))
    }
}
