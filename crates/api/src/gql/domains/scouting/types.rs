use async_graphql::{SimpleObject, ID};

/// A user's privacy/consent settings. Both flags default OFF (G4 / GDPR Art.25 —
/// privacy by default); they are independent — opting into discoverability never
/// implies sharing P/L.
#[derive(SimpleObject, Clone, Debug, Default)]
pub struct PrivacySettings {
    /// Consent to attach identifiable profit/loss to your scouting profile.
    pub share_named_pl: bool,
    /// Consent to be discoverable in opponent lookup (exposes performance stats).
    pub in_scouting_pool: bool,
}

impl From<infra::models::UserPrivacySettingsRow> for PrivacySettings {
    fn from(r: infra::models::UserPrivacySettingsRow) -> Self {
        Self {
            share_named_pl: r.share_named_pl,
            in_scouting_pool: r.in_scouting_pool,
        }
    }
}

/// A pool member matching a search — handle only, no stats until looked up.
#[derive(SimpleObject, Clone, Debug)]
pub struct ScoutingMatch {
    pub user_id: ID,
    pub handle: String,
}

impl From<infra::models::ScoutingMatchRow> for ScoutingMatch {
    fn from(r: infra::models::ScoutingMatchRow) -> Self {
        Self {
            user_id: r.user_id.into(),
            handle: r.handle,
        }
    }
}

/// A looked-up opponent's public scouting profile. Performance stats are shown
/// because they opted into the pool; `net_cents` is present ONLY if they also
/// gave the separate identifiable-P/L consent (G4).
#[derive(SimpleObject, Clone, Debug)]
pub struct ScoutingProfile {
    pub user_id: ID,
    pub handle: String,
    pub tournaments: i32,
    pub itm_percentage: f64,
    pub best_finish: Option<i32>,
    /// Whether this player shares identifiable profit/loss.
    pub shares_pnl: bool,
    /// Net profit/loss in cents — null unless `shares_pnl`.
    pub net_cents: Option<i32>,
}

/// The searcher's free-lookup quota standing.
#[derive(SimpleObject, Clone, Debug)]
pub struct ScoutingQuota {
    pub used: i32,
    pub limit: i32,
    /// True for Pro members who are themselves in the pool (no quota).
    pub unlimited: bool,
}
