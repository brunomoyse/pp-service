use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::gql::error::GqlError;
use infra::db::Db;
use infra::models::ScoutingStatsRow;
use infra::repos::{privacy, pro_entitlements, scouting};

/// Distinct profiles a free user may view per rolling window.
pub const FREE_LOOKUPS: i64 = 5;
const WINDOW_DAYS: i64 = 30;

/// Unlimited lookups are reserved for Pro members who are themselves in the pool
/// (reciprocity: you must be discoverable to get unlimited discovery).
async fn is_unlimited(db: &Db, searcher: Uuid) -> Result<bool, GqlError> {
    Ok(pro_entitlements::is_pro(db, searcher).await?
        && privacy::in_scouting_pool(db, searcher).await?)
}

pub struct QuotaStatus {
    pub used: i64,
    pub limit: i64,
    pub unlimited: bool,
}

pub async fn quota_status(db: &Db, searcher: Uuid) -> Result<QuotaStatus, GqlError> {
    let unlimited = is_unlimited(db, searcher).await?;
    let since = Utc::now() - Duration::days(WINDOW_DAYS);
    let used = scouting::distinct_targets_since(db, searcher, since).await?;
    Ok(QuotaStatus {
        used,
        limit: FREE_LOOKUPS,
        unlimited,
    })
}

pub struct LookupResult {
    pub handle: String,
    pub stats: ScoutingStatsRow,
    pub shares_pnl: bool,
}

/// View a pool member's profile. Enforces consent (target must be in the pool)
/// and the free-lookup quota (a repeat view within the window is free).
pub async fn lookup(db: &Db, searcher: Uuid, target: Uuid) -> Result<LookupResult, GqlError> {
    let handle = scouting::pool_handle(db, target)
        .await?
        .ok_or_else(|| GqlError::new("Player is not in the lookup pool"))?;

    if !is_unlimited(db, searcher).await? {
        let since = Utc::now() - Duration::days(WINDOW_DAYS);
        let already = scouting::has_looked_since(db, searcher, target, since).await?;
        if !already {
            let used = scouting::distinct_targets_since(db, searcher, since).await?;
            if used >= FREE_LOOKUPS {
                return Err(GqlError::new(
                    "Free lookup limit reached — upgrade to Pro for unlimited lookups",
                ));
            }
            scouting::record_lookup(db, searcher, target).await?;
        }
    }

    let stats = scouting::profile_stats(db, target).await?;
    let shares_pnl = privacy::get(db, target)
        .await?
        .map(|s| s.share_named_pl)
        .unwrap_or(false);

    Ok(LookupResult {
        handle,
        stats,
        shares_pnl,
    })
}
