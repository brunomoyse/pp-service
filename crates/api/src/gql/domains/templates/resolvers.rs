use async_graphql::{Context, Object, Result};

use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::blind_structure_templates;

use super::types::{BlindStructureLevel, BlindStructureTemplate};

#[derive(Default)]
pub struct TemplateQuery;

#[Object]
impl TemplateQuery {
    /// Get all available blind structure templates
    async fn blind_structure_templates(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<BlindStructureTemplate>> {
        let state = ctx.data::<AppState>()?;

        let templates = blind_structure_templates::list(&state.db).await?;

        templates
            .into_iter()
            .map(|t| {
                // Parse the JSONB levels into BlindStructureLevel
                let levels: Vec<BlindStructureLevel> =
                    serde_json::from_value(t.levels).gql_err("Parsing template levels failed")?;

                Ok(BlindStructureTemplate {
                    id: t.id.into(),
                    name: t.name,
                    description: t.description,
                    levels,
                    created_at: t.created_at,
                })
            })
            .collect()
    }
}
