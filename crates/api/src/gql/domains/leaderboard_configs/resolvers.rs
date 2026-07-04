use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::permissions::require_club_manager;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::{leaderboard_adjustments, leaderboard_configs};
use infra::scoring::{event_points_with, ScoringFormula as InfraFormula};

use super::types::{
    AddLeaderboardAdjustmentInput, CreateLeaderboardConfigInput, LeaderboardAdjustment,
    LeaderboardConfig, ScoringFormulaInput, ScoringSampleInput, UpdateLeaderboardConfigInput,
};

#[derive(Default)]
pub struct LeaderboardConfigQuery;

#[Object]
impl LeaderboardConfigQuery {
    /// All leagues for a club. Public: standings (`leaderboard(configId:)`) are
    /// public, so the list powering the player app's league selector is too.
    async fn leaderboard_configs(
        &self,
        ctx: &Context<'_>,
        club_id: ID,
    ) -> Result<Vec<LeaderboardConfig>> {
        let club_uuid = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;

        let state = ctx.data::<AppState>()?;
        let rows = leaderboard_configs::list_by_club(&state.db, club_uuid).await?;
        Ok(rows.into_iter().map(LeaderboardConfig::from).collect())
    }

    /// A single league. Public (same visibility as the list and the standings).
    async fn leaderboard_config(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> Result<Option<LeaderboardConfig>> {
        let state = ctx.data::<AppState>()?;
        let config_uuid = Uuid::parse_str(id.as_str()).gql_err("Invalid league ID")?;
        let Some(row) = leaderboard_configs::get_by_id(&state.db, config_uuid).await? else {
            return Ok(None);
        };
        Ok(Some(LeaderboardConfig::from(row)))
    }

    /// Audited manual adjustments for a league, most recent first. Managers only.
    async fn leaderboard_adjustments(
        &self,
        ctx: &Context<'_>,
        config_id: ID,
    ) -> Result<Vec<LeaderboardAdjustment>> {
        let state = ctx.data::<AppState>()?;
        let config = load_config(state, &config_id).await?;
        require_club_manager(ctx, config.club_id).await?;

        let rows = leaderboard_adjustments::list_by_config(&state.db, config.id).await?;
        Ok(rows.into_iter().map(LeaderboardAdjustment::from).collect())
    }

    /// Live preview: points each sample (field size / rank / buy-in) would score
    /// under the given formula. Powers the manager's "1st of 20 @ €50 → X pts" panel.
    async fn preview_scoring(
        &self,
        _ctx: &Context<'_>,
        formula: ScoringFormulaInput,
        samples: Vec<ScoringSampleInput>,
    ) -> Result<Vec<i32>> {
        let f: InfraFormula = formula.into();
        Ok(samples
            .into_iter()
            .map(|s| {
                event_points_with(
                    &f,
                    s.field_size.max(0) as u32,
                    s.rank.max(0) as u32,
                    s.buy_in_cents as f64 / 100.0,
                ) as i32
            })
            .collect())
    }
}

#[derive(Default)]
pub struct LeaderboardConfigMutation;

#[Object]
impl LeaderboardConfigMutation {
    async fn create_leaderboard_config(
        &self,
        ctx: &Context<'_>,
        input: CreateLeaderboardConfigInput,
    ) -> Result<LeaderboardConfig> {
        let club_uuid = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_uuid).await?;
        let state = ctx.data::<AppState>()?;

        let formula: InfraFormula = input.formula.into();
        validate_formula(&formula)?;
        let formula_params = serde_json::to_value(formula).gql_err("Invalid formula")?;
        let membership_mode = input
            .membership_mode
            .map(String::from)
            .unwrap_or_else(|| "all_in_period".to_string());

        let row = leaderboard_configs::create(
            &state.db,
            leaderboard_configs::CreateLeaderboardConfigData {
                club_id: club_uuid,
                name: input.name,
                formula_params,
                membership_mode,
                period_start: input.period_start,
                period_end: input.period_end,
            },
        )
        .await
        .gql_err("Failed to create league")?;

        if input.is_default.unwrap_or(false) {
            leaderboard_configs::set_default(&state.db, club_uuid, row.id).await?;
        }

        // Re-read so is_default reflects any set_default.
        let row = leaderboard_configs::get_by_id(&state.db, row.id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("League not found after create"))?;
        Ok(LeaderboardConfig::from(row))
    }

    async fn update_leaderboard_config(
        &self,
        ctx: &Context<'_>,
        input: UpdateLeaderboardConfigInput,
    ) -> Result<LeaderboardConfig> {
        let state = ctx.data::<AppState>()?;
        let existing = load_config(state, &input.id).await?;
        require_club_manager(ctx, existing.club_id).await?;

        let formula_params = match input.formula {
            Some(f) => {
                let formula: InfraFormula = f.into();
                validate_formula(&formula)?;
                Some(serde_json::to_value(formula).gql_err("Invalid formula")?)
            }
            None => None,
        };

        let updated = leaderboard_configs::update(
            &state.db,
            existing.id,
            leaderboard_configs::UpdateLeaderboardConfigData {
                name: input.name,
                formula_params,
                membership_mode: input.membership_mode.map(String::from),
                period_start: input.period_start,
                period_end: input.period_end,
            },
        )
        .await
        .gql_err("Failed to update league")?
        .ok_or_else(|| async_graphql::Error::new("League not found"))?;

        if input.is_default == Some(true) {
            leaderboard_configs::set_default(&state.db, existing.club_id, updated.id).await?;
        }

        let row = leaderboard_configs::get_by_id(&state.db, updated.id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("League not found"))?;
        Ok(LeaderboardConfig::from(row))
    }

    async fn delete_leaderboard_config(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        let state = ctx.data::<AppState>()?;
        let config = load_config(state, &id).await?;
        require_club_manager(ctx, config.club_id).await?;
        leaderboard_configs::delete(&state.db, config.id)
            .await
            .gql_err("Failed to delete league")
    }

    /// Add an audited manual point adjustment to a league. Managers only.
    async fn add_leaderboard_adjustment(
        &self,
        ctx: &Context<'_>,
        input: AddLeaderboardAdjustmentInput,
    ) -> Result<LeaderboardAdjustment> {
        let state = ctx.data::<AppState>()?;
        let config = load_config(state, &input.config_id).await?;
        let manager = require_club_manager(ctx, config.club_id).await?;

        let reason = input.reason.trim();
        if reason.is_empty() {
            return Err(async_graphql::Error::new("A reason is required"));
        }
        let club_player_id =
            Uuid::parse_str(input.club_player_id.as_str()).gql_err("Invalid player ID")?;
        let created_by = Uuid::parse_str(manager.id.as_str()).ok();

        let row = leaderboard_adjustments::create(
            &state.db,
            config.id,
            club_player_id,
            input.points_delta,
            reason,
            created_by,
        )
        .await
        .gql_err("Failed to add adjustment")?;
        Ok(LeaderboardAdjustment::from(row))
    }

    /// Remove a manual adjustment. Managers of the league's club only.
    async fn remove_leaderboard_adjustment(
        &self,
        ctx: &Context<'_>,
        id: ID,
        config_id: ID,
    ) -> Result<bool> {
        let state = ctx.data::<AppState>()?;
        let config = load_config(state, &config_id).await?;
        require_club_manager(ctx, config.club_id).await?;
        let adj_id = Uuid::parse_str(id.as_str()).gql_err("Invalid adjustment ID")?;
        leaderboard_adjustments::delete(&state.db, adj_id)
            .await
            .gql_err("Failed to remove adjustment")
    }
}

/// Load a league row by GraphQL id (used to resolve the club for authorization).
async fn load_config(
    state: &AppState,
    id: &ID,
) -> Result<infra::repos::leaderboard_configs::LeaderboardConfigRow> {
    let config_uuid = Uuid::parse_str(id.as_str()).gql_err("Invalid league ID")?;
    leaderboard_configs::get_by_id(&state.db, config_uuid)
        .await?
        .ok_or_else(|| async_graphql::Error::new("League not found"))
}

/// Reject obviously broken formulas before they are stored.
fn validate_formula(f: &InfraFormula) -> Result<()> {
    if !f.base_points.is_finite()
        || !f.field_multiplier.is_finite()
        || !f.buyin_multiplier.is_finite()
    {
        return Err(async_graphql::Error::new(
            "Formula coefficients must be finite numbers",
        ));
    }
    if f.cap == 0 {
        return Err(async_graphql::Error::new("Cap must be at least 1"));
    }
    Ok(())
}
