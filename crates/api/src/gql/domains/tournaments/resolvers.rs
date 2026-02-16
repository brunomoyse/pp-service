use async_graphql::{Context, Object, Result};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::auth::permissions::require_club_manager;
use crate::gql::error::ResultExt;
use crate::gql::types::{PaginatedResponse, PaginationInput, Tournament, TournamentStatus};
use crate::state::AppState;
use infra::repos::tournaments::{
    self, CreateTournamentData, TournamentFilter, UpdateTournamentData,
};

use super::types::{CreateTournamentInput, UpdateTournamentInput, UpdateTournamentStatusInput};

#[derive(Default)]
pub struct TournamentQuery;

#[Object]
impl TournamentQuery {
    /// Get tournaments with optional filtering and pagination
    async fn tournaments(
        &self,
        ctx: &Context<'_>,
        club_id: Option<Uuid>,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
        status: Option<TournamentStatus>,
        pagination: Option<PaginationInput>,
    ) -> Result<PaginatedResponse<Tournament>> {
        let state = ctx.data::<AppState>()?;

        let filter = TournamentFilter {
            club_id,
            from,
            to,
            status: status.map(|s| s.into()),
        };

        let page_params = pagination.unwrap_or(PaginationInput {
            limit: Some(50),
            offset: Some(0),
        });
        let limit_offset = page_params.to_limit_offset();

        // Fetch tournaments and total count in parallel
        let (rows, total_count) = tokio::try_join!(
            tournaments::list(&state.db, filter.clone(), Some(limit_offset)),
            tournaments::count(&state.db, filter)
        )
        .gql_err("Database operation failed")?;

        let items: Vec<Tournament> = rows.into_iter().map(Tournament::from).collect();
        let page_size = items.len() as i32;
        let offset = limit_offset.offset as i32;
        let has_next_page = (offset + page_size) < total_count as i32;

        Ok(PaginatedResponse {
            items,
            total_count: total_count as i32,
            page_size,
            offset,
            has_next_page,
        })
    }

    /// Get a single tournament by ID
    async fn tournament(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<Tournament>> {
        let state = ctx.data::<AppState>()?;

        let row = tournaments::get_by_id(&state.db, id)
            .await
            .gql_err("Database operation failed")?;

        Ok(row.map(Tournament::from))
    }
}

#[derive(Default)]
pub struct TournamentMutation;

#[Object]
impl TournamentMutation {
    /// Create a new tournament
    async fn create_tournament(
        &self,
        ctx: &Context<'_>,
        input: CreateTournamentInput,
    ) -> Result<Tournament> {
        let state = ctx.data::<AppState>()?;
        let club_id = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;

        // Check permissions
        let _user = require_club_manager(ctx, club_id).await?;

        // Create tournament data
        let data = CreateTournamentData {
            club_id,
            name: input.name,
            description: input.description,
            start_time: input.start_time,
            end_time: input.end_time,
            buy_in_cents: input.buy_in_cents,
            seat_cap: input.seat_cap,
            early_bird_bonus_chips: input.early_bird_bonus_chips,
            late_registration_level: input.late_registration_level,
        };

        // Create tournament
        let tournament_row = tournaments::create(&state.db, data)
            .await
            .gql_err("Failed to create tournament")?;

        // Handle structure if provided
        if let Some(template_id) = input.template_id {
            let template_uuid =
                Uuid::parse_str(template_id.as_str()).gql_err("Invalid template ID")?;

            // Fetch template
            let template =
                infra::repos::blind_structure_templates::get_by_id(&state.db, template_uuid)
                    .await
                    .gql_err("Failed to fetch template")?
                    .ok_or_else(|| async_graphql::Error::new("Template not found"))?;

            // Deserialize levels from JSON
            let levels: Vec<crate::gql::domains::templates::types::BlindStructureLevel> =
                serde_json::from_value(template.levels)
                    .gql_err("Invalid template levels format")?;

            // Convert to TournamentStructureLevel and add to tournament
            for level in levels {
                let structure_level = infra::repos::tournament_clock::TournamentStructureLevel {
                    level_number: level.level_number,
                    small_blind: level.small_blind,
                    big_blind: level.big_blind,
                    ante: level.ante,
                    duration_minutes: level.duration_minutes,
                    is_break: level.is_break,
                    break_duration_minutes: level.break_duration_minutes,
                };
                infra::repos::tournament_clock::add_structure(
                    &state.db,
                    tournament_row.id,
                    structure_level,
                )
                .await
                .gql_err("Failed to add structure level")?;
            }
        } else if let Some(custom_structure) = input.structure {
            // Add custom structure
            for level_input in custom_structure {
                let structure_level = infra::repos::tournament_clock::TournamentStructureLevel {
                    level_number: level_input.level_number,
                    small_blind: level_input.small_blind,
                    big_blind: level_input.big_blind,
                    ante: level_input.ante,
                    duration_minutes: level_input.duration_minutes,
                    is_break: level_input.is_break,
                    break_duration_minutes: level_input.break_duration_minutes,
                };
                infra::repos::tournament_clock::add_structure(
                    &state.db,
                    tournament_row.id,
                    structure_level,
                )
                .await
                .gql_err("Failed to add structure level")?;
            }
        }

        Ok(Tournament::from(tournament_row))
    }

    /// Update an existing tournament
    async fn update_tournament(
        &self,
        ctx: &Context<'_>,
        input: UpdateTournamentInput,
    ) -> Result<Tournament> {
        let state = ctx.data::<AppState>()?;
        let tournament_id = Uuid::parse_str(input.id.as_str()).gql_err("Invalid tournament ID")?;

        // Get tournament to check club_id
        let existing = tournaments::get_by_id(&state.db, tournament_id)
            .await
            .gql_err("Database operation failed")?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        // Check permissions
        let _user = require_club_manager(ctx, existing.club_id).await?;

        // Update tournament data
        let data = UpdateTournamentData {
            name: input.name,
            description: input.description,
            start_time: input.start_time,
            end_time: input.end_time,
            buy_in_cents: input.buy_in_cents,
            seat_cap: input.seat_cap,
            early_bird_bonus_chips: input.early_bird_bonus_chips,
            late_registration_level: input.late_registration_level,
        };

        let updated_row = tournaments::update(&state.db, tournament_id, data)
            .await
            .gql_err("Failed to update tournament")?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found or already finished"))?;

        // Handle structure updates if provided
        if let Some(template_id) = input.template_id {
            let template_uuid =
                Uuid::parse_str(template_id.as_str()).gql_err("Invalid template ID")?;

            // Fetch template
            let template =
                infra::repos::blind_structure_templates::get_by_id(&state.db, template_uuid)
                    .await
                    .gql_err("Failed to fetch template")?
                    .ok_or_else(|| async_graphql::Error::new("Template not found"))?;

            // Deserialize levels from JSON
            let levels: Vec<crate::gql::domains::templates::types::BlindStructureLevel> =
                serde_json::from_value(template.levels)
                    .gql_err("Invalid template levels format")?;

            // Convert to TournamentStructureLevel
            let structure_levels: Vec<infra::repos::tournament_clock::TournamentStructureLevel> =
                levels
                    .into_iter()
                    .map(
                        |level| infra::repos::tournament_clock::TournamentStructureLevel {
                            level_number: level.level_number,
                            small_blind: level.small_blind,
                            big_blind: level.big_blind,
                            ante: level.ante,
                            duration_minutes: level.duration_minutes,
                            is_break: level.is_break,
                            break_duration_minutes: level.break_duration_minutes,
                        },
                    )
                    .collect();

            // Replace existing structure
            infra::repos::tournament_clock::replace_structures(
                &state.db,
                tournament_id,
                structure_levels,
            )
            .await
            .gql_err("Failed to replace structure")?;
        } else if let Some(custom_structure) = input.structure {
            // Convert input to structure level format
            let levels: Vec<infra::repos::tournament_clock::TournamentStructureLevel> =
                custom_structure
                    .into_iter()
                    .map(
                        |level_input| infra::repos::tournament_clock::TournamentStructureLevel {
                            level_number: level_input.level_number,
                            small_blind: level_input.small_blind,
                            big_blind: level_input.big_blind,
                            ante: level_input.ante,
                            duration_minutes: level_input.duration_minutes,
                            is_break: level_input.is_break,
                            break_duration_minutes: level_input.break_duration_minutes,
                        },
                    )
                    .collect();

            infra::repos::tournament_clock::replace_structures(&state.db, tournament_id, levels)
                .await
                .gql_err("Failed to replace structure")?;
        }

        Ok(Tournament::from(updated_row))
    }

    /// Update tournament live status
    async fn update_tournament_status(
        &self,
        ctx: &Context<'_>,
        input: UpdateTournamentStatusInput,
    ) -> Result<Tournament> {
        let state = ctx.data::<AppState>()?;
        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        // Get tournament to check club_id
        let existing = tournaments::get_by_id(&state.db, tournament_id)
            .await
            .gql_err("Database operation failed")?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        // Check permissions
        let _user = require_club_manager(ctx, existing.club_id).await?;

        // Update live status
        let live_status = input.live_status.into();
        let updated_row = tournaments::update_live_status(&state.db, tournament_id, live_status)
            .await
            .gql_err("Failed to update tournament status")?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        Ok(Tournament::from(updated_row))
    }
}
