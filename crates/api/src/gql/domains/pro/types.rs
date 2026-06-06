use async_graphql::{InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};

/// A Pro entitlement record. A user is Pro while they hold an active, unexpired one.
#[derive(SimpleObject, Clone, Debug)]
pub struct ProEntitlement {
    pub id: ID,
    /// 'club_gift' | 'purchase' | 'manual'
    pub source: String,
    /// 'active' | 'revoked'
    pub status: String,
    pub starts_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub granted_by_club_id: Option<ID>,
    pub notes: Option<String>,
}

impl From<infra::models::ProEntitlementRow> for ProEntitlement {
    fn from(r: infra::models::ProEntitlementRow) -> Self {
        Self {
            id: r.id.into(),
            source: r.source,
            status: r.status,
            starts_at: r.starts_at,
            expires_at: r.expires_at,
            granted_by_club_id: r.granted_by_club_id.map(Into::into),
            notes: r.notes,
        }
    }
}

/// Input for a club gifting Pro to one of its players.
#[derive(InputObject)]
pub struct GrantProEntitlementInput {
    pub app_user_id: ID,
    pub club_id: ID,
    pub expires_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}
