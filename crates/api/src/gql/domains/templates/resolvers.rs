use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::permissions::require_admin;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::blind_structure_templates;
use infra::repos::payout_templates;

use super::types::{
    BlindStructureLevel, BlindStructureTemplate, CreateBlindStructureTemplateInput,
    CreatePayoutTemplateInput, PayoutStructureEntry, PayoutTemplate,
    UpdateBlindStructureTemplateInput, UpdatePayoutTemplateInput,
};

// ── Queries ──

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
                let levels: Vec<BlindStructureLevel> =
                    serde_json::from_value(t.levels).gql_err("Parsing template levels failed")?;

                Ok(BlindStructureTemplate {
                    id: t.id.into(),
                    name: t.name,
                    description: t.description,
                    levels,
                    created_at: t.created_at,
                    updated_at: t.updated_at,
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
                    updated_at: t.updated_at,
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
                    updated_at: t.updated_at,
                })
            })
            .collect()
    }
}

// ── Mutations ──

#[derive(Default)]
pub struct TemplateMutation;

#[Object]
impl TemplateMutation {
    /// Create a new payout template (admin only)
    async fn create_payout_template(
        &self,
        ctx: &Context<'_>,
        input: CreatePayoutTemplateInput,
    ) -> Result<PayoutTemplate> {
        require_admin(ctx).await?;
        let state = ctx.data::<AppState>()?;

        // Validate percentages sum to 100%
        let total: f64 = input.payout_structure.iter().map(|e| e.percentage).sum();
        if (total - 100.0).abs() > 0.01 {
            return Err(async_graphql::Error::new(format!(
                "Payout percentages must sum to 100%, got {:.2}%",
                total
            )));
        }

        let structure_json = serde_json::to_value(
            input
                .payout_structure
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "position": e.position,
                        "percentage": e.percentage,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .gql_err("Failed to serialize payout structure")?;

        let data = payout_templates::CreatePayoutTemplate {
            name: input.name,
            description: input.description,
            min_players: input.min_players,
            max_players: input.max_players,
            payout_structure: structure_json,
        };

        let row = payout_templates::create(&state.db, data).await?;
        let structure: Vec<PayoutStructureEntry> = serde_json::from_value(row.payout_structure)
            .gql_err("Parsing payout structure failed")?;

        Ok(PayoutTemplate {
            id: row.id.into(),
            name: row.name,
            description: row.description,
            min_players: row.min_players,
            max_players: row.max_players,
            payout_structure: structure,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// Update an existing payout template (admin only)
    async fn update_payout_template(
        &self,
        ctx: &Context<'_>,
        input: UpdatePayoutTemplateInput,
    ) -> Result<PayoutTemplate> {
        require_admin(ctx).await?;
        let state = ctx.data::<AppState>()?;

        let id = Uuid::parse_str(input.id.as_str()).gql_err("Invalid template ID")?;

        // Validate percentages sum to 100%
        let total: f64 = input.payout_structure.iter().map(|e| e.percentage).sum();
        if (total - 100.0).abs() > 0.01 {
            return Err(async_graphql::Error::new(format!(
                "Payout percentages must sum to 100%, got {:.2}%",
                total
            )));
        }

        let structure_json = serde_json::to_value(
            input
                .payout_structure
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "position": e.position,
                        "percentage": e.percentage,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .gql_err("Failed to serialize payout structure")?;

        let data = payout_templates::CreatePayoutTemplate {
            name: input.name,
            description: input.description,
            min_players: input.min_players,
            max_players: input.max_players,
            payout_structure: structure_json,
        };

        let row = payout_templates::update(&state.db, id, data).await?;
        let structure: Vec<PayoutStructureEntry> = serde_json::from_value(row.payout_structure)
            .gql_err("Parsing payout structure failed")?;

        Ok(PayoutTemplate {
            id: row.id.into(),
            name: row.name,
            description: row.description,
            min_players: row.min_players,
            max_players: row.max_players,
            payout_structure: structure,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// Delete a payout template (admin only). Fails if template is in use by a tournament.
    async fn delete_payout_template(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        require_admin(ctx).await?;
        let state = ctx.data::<AppState>()?;

        let template_id = Uuid::parse_str(id.as_str()).gql_err("Invalid template ID")?;

        match payout_templates::delete(&state.db, template_id).await {
            Ok(deleted) => Ok(deleted),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("foreign key") || err_str.contains("violates") {
                    Err(async_graphql::Error::new(
                        "Cannot delete template: it is currently in use by one or more tournaments",
                    ))
                } else {
                    Err(e.into())
                }
            }
        }
    }

    /// Create a new blind structure template (admin only)
    async fn create_blind_structure_template(
        &self,
        ctx: &Context<'_>,
        input: CreateBlindStructureTemplateInput,
    ) -> Result<BlindStructureTemplate> {
        require_admin(ctx).await?;
        let state = ctx.data::<AppState>()?;

        let levels_json = serde_json::to_value(
            input
                .levels
                .iter()
                .map(|l| {
                    serde_json::json!({
                        "levelNumber": l.level_number,
                        "smallBlind": l.small_blind,
                        "bigBlind": l.big_blind,
                        "ante": l.ante,
                        "durationMinutes": l.duration_minutes,
                        "isBreak": l.is_break,
                        "breakDurationMinutes": l.break_duration_minutes,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .gql_err("Failed to serialize levels")?;

        let data = blind_structure_templates::CreateBlindStructureTemplate {
            name: input.name,
            description: input.description,
            levels: levels_json,
        };

        let row = blind_structure_templates::create(&state.db, data).await?;
        let levels: Vec<BlindStructureLevel> =
            serde_json::from_value(row.levels).gql_err("Parsing template levels failed")?;

        Ok(BlindStructureTemplate {
            id: row.id.into(),
            name: row.name,
            description: row.description,
            levels,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// Update an existing blind structure template (admin only)
    async fn update_blind_structure_template(
        &self,
        ctx: &Context<'_>,
        input: UpdateBlindStructureTemplateInput,
    ) -> Result<BlindStructureTemplate> {
        require_admin(ctx).await?;
        let state = ctx.data::<AppState>()?;

        let id = Uuid::parse_str(input.id.as_str()).gql_err("Invalid template ID")?;

        let levels_json = serde_json::to_value(
            input
                .levels
                .iter()
                .map(|l| {
                    serde_json::json!({
                        "levelNumber": l.level_number,
                        "smallBlind": l.small_blind,
                        "bigBlind": l.big_blind,
                        "ante": l.ante,
                        "durationMinutes": l.duration_minutes,
                        "isBreak": l.is_break,
                        "breakDurationMinutes": l.break_duration_minutes,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .gql_err("Failed to serialize levels")?;

        let data = blind_structure_templates::CreateBlindStructureTemplate {
            name: input.name,
            description: input.description,
            levels: levels_json,
        };

        let row = blind_structure_templates::update(&state.db, id, data).await?;
        let levels: Vec<BlindStructureLevel> =
            serde_json::from_value(row.levels).gql_err("Parsing template levels failed")?;

        Ok(BlindStructureTemplate {
            id: row.id.into(),
            name: row.name,
            description: row.description,
            levels,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// Delete a blind structure template (admin only)
    async fn delete_blind_structure_template(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        require_admin(ctx).await?;
        let state = ctx.data::<AppState>()?;

        let template_id = Uuid::parse_str(id.as_str()).gql_err("Invalid template ID")?;

        match blind_structure_templates::delete(&state.db, template_id).await {
            Ok(deleted) => Ok(deleted),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("foreign key") || err_str.contains("violates") {
                    Err(async_graphql::Error::new(
                        "Cannot delete template: it is currently in use by one or more tournaments",
                    ))
                } else {
                    Err(e.into())
                }
            }
        }
    }
}
