use async_graphql::SimpleObject;

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
