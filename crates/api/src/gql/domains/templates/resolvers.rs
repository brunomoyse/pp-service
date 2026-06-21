use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::permissions::require_club_manager;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::blind_structure_templates;
use infra::repos::payout_templates;

use super::types::{
    BlindStructureLevel, BlindStructureTemplate, CreateBlindStructureTemplateInput,
    CreatePayoutTemplateInput, PayoutStructureEntry, PayoutTemplate,
    UpdateBlindStructureTemplateInput, UpdatePayoutTemplateInput,
};

/// Build the GraphQL `PayoutTemplate` from a DB row (parses the JSONB structure).
fn payout_template_from_row(row: infra::models::PayoutTemplateRow) -> Result<PayoutTemplate> {
    let payout_structure: Vec<PayoutStructureEntry> =
        serde_json::from_value(row.payout_structure).gql_err("Parsing payout structure failed")?;
    Ok(PayoutTemplate {
        id: row.id.into(),
        club_id: row.club_id.into(),
        name: row.name,
        description: row.description,
        min_players: row.min_players,
        max_players: row.max_players,
        payout_structure,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// Build the GraphQL `BlindStructureTemplate` from a DB row (parses the JSONB levels).
fn blind_template_from_row(
    row: infra::models::BlindStructureTemplateRow,
) -> Result<BlindStructureTemplate> {
    let levels: Vec<BlindStructureLevel> =
        serde_json::from_value(row.levels).gql_err("Parsing template levels failed")?;
    Ok(BlindStructureTemplate {
        id: row.id.into(),
        club_id: row.club_id.into(),
        name: row.name,
        description: row.description,
        levels,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

// ── Queries ──

#[derive(Default)]
pub struct TemplateQuery;

#[Object]
impl TemplateQuery {
    /// Get the blind structure templates owned by a club (manager only).
    async fn blind_structure_templates(
        &self,
        ctx: &Context<'_>,
        club_id: ID,
    ) -> Result<Vec<BlindStructureTemplate>> {
        let state = ctx.data::<AppState>()?;
        let club_uuid = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_uuid).await?;

        let templates = blind_structure_templates::list_by_club(&state.db, club_uuid).await?;

        templates.into_iter().map(blind_template_from_row).collect()
    }

    /// Get the payout templates owned by a club (manager only).
    async fn payout_templates(
        &self,
        ctx: &Context<'_>,
        club_id: ID,
    ) -> Result<Vec<PayoutTemplate>> {
        let state = ctx.data::<AppState>()?;
        let club_uuid = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_uuid).await?;

        let templates = payout_templates::list_by_club(&state.db, club_uuid).await?;
        templates
            .into_iter()
            .map(payout_template_from_row)
            .collect()
    }

    /// Get a club's payout templates suitable for a given player count (manager only).
    async fn suitable_payout_templates(
        &self,
        ctx: &Context<'_>,
        club_id: ID,
        player_count: i32,
    ) -> Result<Vec<PayoutTemplate>> {
        let state = ctx.data::<AppState>()?;
        let club_uuid = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_uuid).await?;

        let templates =
            payout_templates::find_suitable_templates(&state.db, club_uuid, player_count).await?;
        templates
            .into_iter()
            .map(payout_template_from_row)
            .collect()
    }

    /// Preview a decaying "pay the top N%" payout structure for a field size,
    /// without saving anything. `percent_paid` defaults to 15%. The result drops
    /// straight into a payout template's structure (Auto mode in the editor).
    async fn auto_payout_preview(
        &self,
        _ctx: &Context<'_>,
        num_players: i32,
        percent_paid: Option<f64>,
    ) -> Result<Vec<PayoutStructureEntry>> {
        if num_players < 1 {
            return Err(async_graphql::Error::new("num_players must be at least 1"));
        }
        let percent = percent_paid.unwrap_or(super::payout_curve::DEFAULT_PERCENT_PAID);
        if percent <= 0.0 || percent > 100.0 {
            return Err(async_graphql::Error::new(
                "percent_paid must be between 0 and 100",
            ));
        }
        Ok(super::payout_curve::auto_payout_structure(
            num_players,
            percent,
        ))
    }
}

/// Validate that payout percentages sum to 100%.
fn validate_payout_sum(structure: &[super::types::PayoutStructureEntryInput]) -> Result<()> {
    let total: f64 = structure.iter().map(|e| e.percentage).sum();
    if (total - 100.0).abs() > 0.01 {
        return Err(async_graphql::Error::new(format!(
            "Payout percentages must sum to 100%, got {:.2}%",
            total
        )));
    }
    Ok(())
}

/// Serialize the payout structure input into the stored JSONB shape.
fn payout_structure_json(
    structure: &[super::types::PayoutStructureEntryInput],
) -> Result<serde_json::Value> {
    serde_json::to_value(
        structure
            .iter()
            .map(|e| serde_json::json!({ "position": e.position, "percentage": e.percentage }))
            .collect::<Vec<_>>(),
    )
    .gql_err("Failed to serialize payout structure")
}

/// Serialize the blind levels input into the stored JSONB shape.
fn blind_levels_json(
    levels: &[super::types::BlindStructureLevelInput],
) -> Result<serde_json::Value> {
    serde_json::to_value(
        levels
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
    .gql_err("Failed to serialize levels")
}

// ── Mutations ──

#[derive(Default)]
pub struct TemplateMutation;

#[Object]
impl TemplateMutation {
    /// Create a payout template for a club (club manager only).
    async fn create_payout_template(
        &self,
        ctx: &Context<'_>,
        input: CreatePayoutTemplateInput,
    ) -> Result<PayoutTemplate> {
        let state = ctx.data::<AppState>()?;
        let club_id = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_id).await?;

        validate_payout_sum(&input.payout_structure)?;
        let structure_json = payout_structure_json(&input.payout_structure)?;

        let data = payout_templates::CreatePayoutTemplate {
            club_id,
            name: input.name,
            description: input.description,
            min_players: input.min_players,
            max_players: input.max_players,
            payout_structure: structure_json,
        };

        let row = payout_templates::create(&state.db, data).await?;
        payout_template_from_row(row)
    }

    /// Update one of the club's payout templates (club manager only).
    async fn update_payout_template(
        &self,
        ctx: &Context<'_>,
        input: UpdatePayoutTemplateInput,
    ) -> Result<PayoutTemplate> {
        let state = ctx.data::<AppState>()?;
        let id = Uuid::parse_str(input.id.as_str()).gql_err("Invalid template ID")?;

        // Authorize against the template's owning club.
        let existing = payout_templates::get_by_id(&state.db, id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Payout template not found"))?;
        require_club_manager(ctx, existing.club_id).await?;

        validate_payout_sum(&input.payout_structure)?;
        let structure_json = payout_structure_json(&input.payout_structure)?;

        let data = payout_templates::UpdatePayoutTemplate {
            name: input.name,
            description: input.description,
            min_players: input.min_players,
            max_players: input.max_players,
            payout_structure: structure_json,
        };

        let row = payout_templates::update(&state.db, id, data).await?;
        payout_template_from_row(row)
    }

    /// Delete one of the club's payout templates (club manager only).
    async fn delete_payout_template(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        let state = ctx.data::<AppState>()?;
        let template_id = Uuid::parse_str(id.as_str()).gql_err("Invalid template ID")?;

        let existing = payout_templates::get_by_id(&state.db, template_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Payout template not found"))?;
        require_club_manager(ctx, existing.club_id).await?;

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

    /// Create a blind structure template for a club (club manager only).
    async fn create_blind_structure_template(
        &self,
        ctx: &Context<'_>,
        input: CreateBlindStructureTemplateInput,
    ) -> Result<BlindStructureTemplate> {
        let state = ctx.data::<AppState>()?;
        let club_id = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_id).await?;

        let levels_json = blind_levels_json(&input.levels)?;

        let data = blind_structure_templates::CreateBlindStructureTemplate {
            club_id,
            name: input.name,
            description: input.description,
            levels: levels_json,
        };

        let row = blind_structure_templates::create(&state.db, data).await?;
        blind_template_from_row(row)
    }

    /// Update one of the club's blind structure templates (club manager only).
    async fn update_blind_structure_template(
        &self,
        ctx: &Context<'_>,
        input: UpdateBlindStructureTemplateInput,
    ) -> Result<BlindStructureTemplate> {
        let state = ctx.data::<AppState>()?;
        let id = Uuid::parse_str(input.id.as_str()).gql_err("Invalid template ID")?;

        let existing = blind_structure_templates::get_by_id(&state.db, id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Blind structure template not found"))?;
        require_club_manager(ctx, existing.club_id).await?;

        let levels_json = blind_levels_json(&input.levels)?;

        let data = blind_structure_templates::UpdateBlindStructureTemplate {
            name: input.name,
            description: input.description,
            levels: levels_json,
        };

        let row = blind_structure_templates::update(&state.db, id, data).await?;
        blind_template_from_row(row)
    }

    /// Delete one of the club's blind structure templates (club manager only).
    async fn delete_blind_structure_template(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        let state = ctx.data::<AppState>()?;
        let template_id = Uuid::parse_str(id.as_str()).gql_err("Invalid template ID")?;

        let existing = blind_structure_templates::get_by_id(&state.db, template_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Blind structure template not found"))?;
        require_club_manager(ctx, existing.club_id).await?;

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
