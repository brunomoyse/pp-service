use uuid::Uuid;

use crate::gql::error::GqlError;
use infra::db::Db;
use infra::models::ScoutingStatsRow;
use infra::repos::{privacy, scouting};

pub struct QuotaStatus {
    pub used: i64,
    pub limit: i64,
    pub unlimited: bool,
}

pub async fn quota_status(_db: &Db, _searcher: Uuid) -> Result<QuotaStatus, GqlError> {
    Ok(QuotaStatus {
        used: 0,
        limit: 0,
        unlimited: true,
    })
}

pub struct LookupResult {
    pub handle: String,
    pub stats: ScoutingStatsRow,
    pub shares_pnl: bool,
}

/// View a pool member's profile. Enforces consent (target must be in the pool).
/// Lookups are unlimited for all users.
pub async fn lookup(db: &Db, _searcher: Uuid, target: Uuid) -> Result<LookupResult, GqlError> {
    let handle = scouting::pool_handle(db, target)
        .await?
        .ok_or_else(|| GqlError::new("Player is not in the lookup pool"))?;

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
