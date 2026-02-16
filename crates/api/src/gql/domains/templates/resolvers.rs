use async_graphql::{Context, Object, Result};

use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::blind_structure_templates;
use infra::repos::payout_templates;

use super::types::{
    BlindStructureLevel, BlindStructureTemplate, PayoutStructureEntry, PayoutTemplate,
};

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

    /// Get all available payout templates
    async fn payout_templates(&self, ctx: &Context<'_>) -> Result<Vec<PayoutTemplate>> {
        let state = ctx.data::<AppState>()?;
        let templates = payout_templates::list(&state.db).await?;
        templates
            .into_iter()
            .map(|t| {
                let structure: Vec<PayoutStructureEntry> =
                    serde_json::from_value(t.payout_structure)
                        .gql_err("Parsing payout structure failed")?;
                Ok(PayoutTemplate {
                    id: t.id.into(),
                    name: t.name,
                    description: t.description,
                    min_players: t.min_players,
                    max_players: t.max_players,
                    payout_structure: structure,
                    created_at: t.created_at,
                })
            })
            .collect()
    }

    /// Get payout templates suitable for a given player count
    async fn suitable_payout_templates(
        &self,
        ctx: &Context<'_>,
        player_count: i32,
    ) -> Result<Vec<PayoutTemplate>> {
        let state = ctx.data::<AppState>()?;
        let templates = payout_templates::find_suitable_templates(&state.db, player_count).await?;
        templates
            .into_iter()
            .map(|t| {
                let structure: Vec<PayoutStructureEntry> =
                    serde_json::from_value(t.payout_structure)
                        .gql_err("Parsing payout structure failed")?;
                Ok(PayoutTemplate {
                    id: t.id.into(),
                    name: t.name,
                    description: t.description,
                    min_players: t.min_players,
                    max_players: t.max_players,
                    payout_structure: structure,
                    created_at: t.created_at,
                })
            })
            .collect()
    }
}
