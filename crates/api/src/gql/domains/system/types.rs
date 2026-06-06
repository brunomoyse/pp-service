use async_graphql::SimpleObject;

use crate::features::{is_enabled, Feature};

/// Which optional features are enabled on this server, so clients can hide UI
/// for anything that is gated off (or pending legal sign-off).
#[derive(SimpleObject, Clone, Debug)]
pub struct FeatureFlags {
    pub notes: bool,
    pub pro_account: bool,
    pub predictions: bool,
    pub cosmetics: bool,
    pub public_stats: bool,
}

impl FeatureFlags {
    pub fn current() -> Self {
        Self {
            notes: is_enabled(Feature::Notes),
            pro_account: is_enabled(Feature::ProAccount),
            predictions: is_enabled(Feature::Predictions),
            cosmetics: is_enabled(Feature::Cosmetics),
            public_stats: is_enabled(Feature::PublicStats),
        }
    }
}
