use async_graphql::{Object, Result};

use super::types::FeatureFlags;

#[derive(Default)]
pub struct SystemQuery;

#[Object]
impl SystemQuery {
    /// Which optional features are currently enabled on this server.
    async fn feature_flags(&self) -> Result<FeatureFlags> {
        Ok(FeatureFlags::current())
    }
}
