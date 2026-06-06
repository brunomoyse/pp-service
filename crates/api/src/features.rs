//! Lightweight, env-driven feature flags.
//!
//! Everything touching the gamification / Pro / economy / public-stats work
//! ships behind one of these flags until legal sign-off and rollout. All flags
//! default OFF. Set e.g. `FEATURE_NOTES=true` to enable.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    Notes,
    ProAccount,
    Predictions,
    Cosmetics,
    PublicStats,
}

impl Feature {
    const fn env_key(self) -> &'static str {
        match self {
            Feature::Notes => "FEATURE_NOTES",
            Feature::ProAccount => "FEATURE_PRO_ACCOUNT",
            Feature::Predictions => "FEATURE_PREDICTIONS",
            Feature::Cosmetics => "FEATURE_COSMETICS",
            Feature::PublicStats => "FEATURE_PUBLIC_STATS",
        }
    }
}

fn env_truthy(key: &str) -> bool {
    std::env::var(key)
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// Whether a feature is currently enabled on this server.
pub fn is_enabled(feature: Feature) -> bool {
    env_truthy(feature.env_key())
}

/// Resolver guard: error out when a feature is disabled.
pub fn require_feature(feature: Feature) -> async_graphql::Result<()> {
    if is_enabled(feature) {
        Ok(())
    } else {
        Err(async_graphql::Error::new(
            "This feature is not currently available",
        ))
    }
}
