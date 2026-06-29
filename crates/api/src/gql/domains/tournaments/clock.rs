use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use std::str::FromStr;
use uuid::Uuid;

use crate::auth::permissions::require_club_manager;
use crate::gql::common::helpers::get_club_id_for_tournament;
use crate::gql::subscriptions::publish_clock_update;
use crate::gql::types::{ClockStatus, TournamentClock, TournamentStructure};
use crate::AppState;
use infra::repos::tournament_clock::{self, ClockStatus as InfraClockStatus};

/// Fire-and-forget: record a clock state change in the tournament activity log.
/// Mirrors the logging used by the other domains; failures are swallowed inside
/// `log_and_publish` so they never fail the originating mutation.
fn log_clock_event(
    db: &sqlx::PgPool,
    tournament_id: Uuid,
    action: &'static str,
    actor_id: Option<Uuid>,
    metadata: serde_json::Value,
) {
    let db = db.clone();
    tokio::spawn(async move {
        crate::gql::domains::activity_log::log_and_publish(
            &db,
            tournament_id,
            "clock",
            action,
            actor_id,
            None,
            metadata,
        )
        .await;
    });
}

/// After a level advance, close late registration if the tournament has just
/// passed its configured `late_registration_level`. Shared by the manual
/// advance mutation and the clock service's auto-advance loop so the transition
/// happens no matter how the level was advanced.
///
/// Returns `true` when the status was flipped to `InProgress`. Best-effort: the
/// caller decides how to surface errors; this never panics.
pub async fn close_late_registration_if_due(
    pool: &sqlx::PgPool,
    tournament_id: Uuid,
    new_level: i32,
    actor_id: Option<Uuid>,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    use infra::repos::tournaments::{self, TournamentLiveStatus};

    let tournament = tournaments::get_by_id(pool, tournament_id)
        .await?
        .ok_or("Tournament not found")?;

    // Only act while late registration is actually open.
    if tournament.live_status != TournamentLiveStatus::LateRegistration {
        return Ok(false);
    }

    // Only act when a late-registration level is configured.
    let Some(late_reg_level) = tournament.late_registration_level else {
        return Ok(false);
    };

    // Late reg closes at the END of the configured level, i.e. once we have
    // advanced into the following level.
    if new_level <= late_reg_level {
        return Ok(false);
    }

    let updated =
        tournaments::update_live_status(pool, tournament_id, TournamentLiveStatus::InProgress)
            .await?
            .ok_or("Tournament not found")?;

    crate::gql::domains::activity_log::log_and_publish(
        pool,
        tournament_id,
        "tournament",
        "status_changed",
        actor_id,
        None,
        serde_json::json!({
            "from_status": format!("{:?}", TournamentLiveStatus::LateRegistration),
            "to_status": format!("{:?}", updated.live_status),
            "auto": true,
        }),
    )
    .await;

    Ok(true)
}

/// Helper function to get next structure for a tournament
async fn get_next_structure(
    pool: &sqlx::PgPool,
    tournament_id: Uuid,
    current_level: i32,
) -> Option<TournamentStructure> {
    tournament_clock::get_all_structures(pool, tournament_id)
        .await
        .ok()
        .and_then(|structures| {
            structures
                .into_iter()
                .find(|s| s.level_number == current_level + 1)
                .map(TournamentStructure::from)
        })
}

/// Helper function to create TournamentClock with all required fields
fn create_tournament_clock(
    clock_row: &infra::models::TournamentClockRow,
    structure: Option<&infra::models::TournamentStructureRow>,
    next_structure: Option<TournamentStructure>,
    time_remaining: Option<i64>,
    total_pause_seconds: i64,
    status: ClockStatus,
) -> TournamentClock {
    TournamentClock {
        id: clock_row.id.into(),
        tournament_id: clock_row.tournament_id.into(),
        status,
        current_level: clock_row.current_level,
        time_remaining_seconds: time_remaining,
        level_started_at: clock_row.level_started_at,
        level_end_time: clock_row.level_end_time,
        total_pause_duration_seconds: total_pause_seconds,
        auto_advance: clock_row.auto_advance,
        current_structure: structure.map(|s| TournamentStructure::from(s.clone())),
        next_structure,
        // Additional fields from ClockUpdate
        small_blind: structure.map(|s| s.small_blind),
        big_blind: structure.map(|s| s.big_blind),
        ante: structure.map(|s| s.ante),
        is_break: structure.map(|s| s.is_break),
        level_duration_minutes: structure.map(|s| s.duration_minutes),
    }
}

#[derive(Default)]
pub struct TournamentClockQuery;

#[Object]
impl TournamentClockQuery {
    /// Get tournament clock state
    pub async fn tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<Option<TournamentClock>> {
        let state = ctx.data::<AppState>()?;
        let tournament_id: Uuid = tournament_id.parse()?;

        if let Some(clock_row) = tournament_clock::get_clock(&state.db, tournament_id).await? {
            let structure = tournament_clock::get_current_structure(&state.db, tournament_id)
                .await
                .ok();
            let next_structure =
                get_next_structure(&state.db, tournament_id, clock_row.current_level).await;

            // Calculate time remaining
            let time_remaining = if let Ok(status) =
                InfraClockStatus::from_str(&clock_row.clock_status)
            {
                match status {
                    InfraClockStatus::Running => {
                        if let Some(end_time) = clock_row.level_end_time {
                            let remaining = end_time - Utc::now();
                            Some(remaining.num_seconds().max(0))
                        } else {
                            None
                        }
                    }
                    InfraClockStatus::Paused => {
                        if let (Some(end_time), Some(pause_start)) =
                            (clock_row.level_end_time, clock_row.pause_started_at)
                        {
                            let remaining = end_time - pause_start;
                            Some(remaining.num_seconds().max(0))
                        } else {
                            None
                        }
                    }
                    InfraClockStatus::Stopped => {
                        // Show full duration of current level when stopped
                        // Use already fetched structure or fetch it
                        if let Some(s) = &structure {
                            Some((s.duration_minutes as i64) * 60)
                        } else if let Ok(current_structure) =
                            tournament_clock::get_current_structure(&state.db, tournament_id).await
                        {
                            Some((current_structure.duration_minutes as i64) * 60)
                        } else {
                            None
                        }
                    }
                }
            } else {
                None
            };

            // Convert PgInterval to seconds
            let total_pause_seconds = clock_row.total_pause_duration.microseconds / 1_000_000;

            Ok(Some(TournamentClock {
                id: clock_row.id.into(),
                tournament_id: clock_row.tournament_id.into(),
                status: InfraClockStatus::from_str(&clock_row.clock_status)
                    .ok()
                    .unwrap_or(InfraClockStatus::Stopped)
                    .into(),
                current_level: clock_row.current_level,
                time_remaining_seconds: time_remaining,
                level_started_at: clock_row.level_started_at,
                level_end_time: clock_row.level_end_time,
                total_pause_duration_seconds: total_pause_seconds,
                auto_advance: clock_row.auto_advance,
                current_structure: structure
                    .as_ref()
                    .map(|s| TournamentStructure::from(s.clone())),
                next_structure,
                // Additional fields from ClockUpdate
                small_blind: structure.as_ref().map(|s| s.small_blind),
                big_blind: structure.as_ref().map(|s| s.big_blind),
                ante: structure.as_ref().map(|s| s.ante),
                is_break: structure.as_ref().map(|s| s.is_break),
                level_duration_minutes: structure.as_ref().map(|s| s.duration_minutes),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get tournament structure levels
    pub async fn tournament_structure(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<Vec<TournamentStructure>> {
        let state = ctx.data::<AppState>()?;
        let tournament_id: Uuid = tournament_id.parse()?;

        let structures = tournament_clock::get_all_structures(&state.db, tournament_id).await?;

        Ok(structures
            .into_iter()
            .map(TournamentStructure::from)
            .collect())
    }
}

#[derive(Default)]
pub struct TournamentClockMutation;

#[Object]
impl TournamentClockMutation {
    /// Initialize tournament clock
    pub async fn create_tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<TournamentClock> {
        let state = ctx.data::<AppState>()?;
        let tournament_id: Uuid = tournament_id.parse()?;

        let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;
        let _manager = require_club_manager(ctx, club_id).await?;

        let clock_row = tournament_clock::create_clock(&state.db, tournament_id).await?;
        let structure = tournament_clock::get_current_structure(&state.db, tournament_id)
            .await
            .ok();
        let next_structure =
            get_next_structure(&state.db, tournament_id, clock_row.current_level).await;

        // Show full duration of first level when clock is created (stopped state)
        let time_remaining_seconds = structure.as_ref().map(|s| (s.duration_minutes as i64) * 60);

        let clock = TournamentClock {
            id: clock_row.id.into(),
            tournament_id: clock_row.tournament_id.into(),
            status: InfraClockStatus::from_str(&clock_row.clock_status)
                .ok()
                .unwrap_or(InfraClockStatus::Stopped)
                .into(),
            current_level: clock_row.current_level,
            time_remaining_seconds,
            level_started_at: clock_row.level_started_at,
            level_end_time: clock_row.level_end_time,
            total_pause_duration_seconds: 0,
            auto_advance: clock_row.auto_advance,
            current_structure: structure
                .as_ref()
                .map(|s| TournamentStructure::from(s.clone())),
            next_structure,
            // Additional fields from ClockUpdate
            small_blind: structure.as_ref().map(|s| s.small_blind),
            big_blind: structure.as_ref().map(|s| s.big_blind),
            ante: structure.as_ref().map(|s| s.ante),
            is_break: structure.as_ref().map(|s| s.is_break),
            level_duration_minutes: structure.as_ref().map(|s| s.duration_minutes),
        };

        // Publish to subscription channel
        publish_clock_update(tournament_id, clock.clone());

        Ok(clock)
    }

    /// Start tournament clock
    pub async fn start_tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<TournamentClock> {
        let state = ctx.data::<AppState>()?;
        let tournament_id: Uuid = tournament_id.parse()?;

        let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;
        let manager = require_club_manager(ctx, club_id).await?;

        let clock_row =
            tournament_clock::start_clock(&state.db, tournament_id, Some(manager.id.parse()?))
                .await?;
        log_clock_event(
            &state.db,
            tournament_id,
            "start",
            manager.id.parse().ok(),
            serde_json::json!({}),
        );
        let structure = tournament_clock::get_current_structure(&state.db, tournament_id)
            .await
            .ok();
        let next_structure =
            get_next_structure(&state.db, tournament_id, clock_row.current_level).await;

        // Calculate time remaining
        let time_remaining = if let Some(end_time) = clock_row.level_end_time {
            let remaining = end_time - Utc::now();
            Some(remaining.num_seconds().max(0))
        } else {
            None
        };

        let clock = create_tournament_clock(
            &clock_row,
            structure.as_ref(),
            next_structure,
            time_remaining,
            0,
            ClockStatus::Running,
        );

        // Publish to subscription channel
        publish_clock_update(tournament_id, clock.clone());

        Ok(clock)
    }

    /// Pause tournament clock
    pub async fn pause_tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<TournamentClock> {
        let state = ctx.data::<AppState>()?;
        let tournament_id: Uuid = tournament_id.parse()?;

        let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;
        let manager = require_club_manager(ctx, club_id).await?;

        let clock_row =
            tournament_clock::pause_clock(&state.db, tournament_id, Some(manager.id.parse()?))
                .await?;
        log_clock_event(
            &state.db,
            tournament_id,
            "pause",
            manager.id.parse().ok(),
            serde_json::json!({}),
        );
        let structure = tournament_clock::get_current_structure(&state.db, tournament_id)
            .await
            .ok();
        let next_structure =
            get_next_structure(&state.db, tournament_id, clock_row.current_level).await;

        // Calculate time remaining at pause
        let time_remaining = if let (Some(end_time), Some(pause_start)) =
            (clock_row.level_end_time, clock_row.pause_started_at)
        {
            let remaining = end_time - pause_start;
            Some(remaining.num_seconds().max(0))
        } else {
            None
        };

        let total_pause_seconds = clock_row.total_pause_duration.microseconds / 1_000_000;

        let clock = create_tournament_clock(
            &clock_row,
            structure.as_ref(),
            next_structure,
            time_remaining,
            total_pause_seconds,
            ClockStatus::Paused,
        );

        // Publish to subscription channel
        publish_clock_update(tournament_id, clock.clone());

        Ok(clock)
    }

    /// Resume tournament clock
    pub async fn resume_tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<TournamentClock> {
        let state = ctx.data::<AppState>()?;
        let tournament_id: Uuid = tournament_id.parse()?;

        let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;
        let manager = require_club_manager(ctx, club_id).await?;

        let clock_row =
            tournament_clock::resume_clock(&state.db, tournament_id, Some(manager.id.parse()?))
                .await?;
        log_clock_event(
            &state.db,
            tournament_id,
            "resume",
            manager.id.parse().ok(),
            serde_json::json!({}),
        );
        let structure = tournament_clock::get_current_structure(&state.db, tournament_id)
            .await
            .ok();
        let next_structure =
            get_next_structure(&state.db, tournament_id, clock_row.current_level).await;

        // Calculate time remaining
        let time_remaining = if let Some(end_time) = clock_row.level_end_time {
            let remaining = end_time - Utc::now();
            Some(remaining.num_seconds().max(0))
        } else {
            None
        };

        let total_pause_seconds = clock_row.total_pause_duration.microseconds / 1_000_000;

        let clock = create_tournament_clock(
            &clock_row,
            structure.as_ref(),
            next_structure,
            time_remaining,
            total_pause_seconds,
            ClockStatus::Running,
        );

        // Publish to subscription channel
        publish_clock_update(tournament_id, clock.clone());

        Ok(clock)
    }

    /// Manually advance to next level
    pub async fn advance_tournament_level(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<TournamentClock> {
        let state = ctx.data::<AppState>()?;
        let tournament_id: Uuid = tournament_id.parse()?;

        // Get tournament to find club_id for authorization
        let tournament = infra::repos::tournaments::get_by_id(&state.db, tournament_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        // Require club-specific manager authorization
        let manager =
            crate::auth::permissions::require_club_manager(ctx, tournament.club_id).await?;

        let clock_row = tournament_clock::advance_level(
            &state.db,
            tournament_id,
            false,
            Some(manager.id.parse()?),
        )
        .await?;
        log_clock_event(
            &state.db,
            tournament_id,
            "manual_advance",
            manager.id.parse().ok(),
            serde_json::json!({ "level_number": clock_row.current_level }),
        );

        // Auto-close late registration if this advance passed the configured
        // level. Done before publishing the clock update so a client refetch
        // triggered by that update sees the new status. Best-effort.
        if let Err(e) = close_late_registration_if_due(
            &state.db,
            tournament_id,
            clock_row.current_level,
            manager.id.parse().ok(),
        )
        .await
        {
            tracing::warn!(
                tournament_id = %tournament_id,
                error = %e,
                "Failed to auto-close late registration after manual advance",
            );
        }

        let structure = tournament_clock::get_current_structure(&state.db, tournament_id)
            .await
            .ok();
        let next_structure =
            get_next_structure(&state.db, tournament_id, clock_row.current_level).await;

        // Calculate time remaining for new level
        let time_remaining = if let Some(end_time) = clock_row.level_end_time {
            let remaining = end_time - Utc::now();
            Some(remaining.num_seconds().max(0))
        } else {
            None
        };

        let status = InfraClockStatus::from_str(&clock_row.clock_status)
            .ok()
            .unwrap_or(InfraClockStatus::Stopped)
            .into();

        // Convert PgInterval to seconds for total pause duration
        let total_pause_seconds = clock_row.total_pause_duration.microseconds / 1_000_000;

        let clock = create_tournament_clock(
            &clock_row,
            structure.as_ref(),
            next_structure,
            time_remaining,
            total_pause_seconds,
            status,
        );

        // Publish to subscription channel
        publish_clock_update(tournament_id, clock.clone());

        Ok(clock)
    }

    /// Manually revert to previous level
    pub async fn revert_tournament_level(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<TournamentClock> {
        let state = ctx.data::<AppState>()?;
        let tournament_id: Uuid = tournament_id.parse()?;

        let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;
        let manager = require_club_manager(ctx, club_id).await?;

        let clock_row =
            tournament_clock::revert_level(&state.db, tournament_id, Some(manager.id.parse()?))
                .await
                .map_err(|e| match e {
                    sqlx::Error::RowNotFound => async_graphql::Error::new(
                        "Reverting level failed: tournament is already at level 1",
                    ),
                    other => async_graphql::Error::new(format!("Reverting level failed: {other}")),
                })?;
        log_clock_event(
            &state.db,
            tournament_id,
            "manual_revert",
            manager.id.parse().ok(),
            serde_json::json!({ "level_number": clock_row.current_level }),
        );
        let structure = tournament_clock::get_current_structure(&state.db, tournament_id)
            .await
            .ok();
        let next_structure =
            get_next_structure(&state.db, tournament_id, clock_row.current_level).await;

        // Calculate time remaining for reverted level
        let time_remaining = if let Some(end_time) = clock_row.level_end_time {
            let remaining = end_time - Utc::now();
            Some(remaining.num_seconds().max(0))
        } else {
            None
        };

        let status = InfraClockStatus::from_str(&clock_row.clock_status)
            .ok()
            .unwrap_or(InfraClockStatus::Stopped)
            .into();

        // Convert PgInterval to seconds for total pause duration
        let total_pause_seconds = clock_row.total_pause_duration.microseconds / 1_000_000;

        let clock = create_tournament_clock(
            &clock_row,
            structure.as_ref(),
            next_structure,
            time_remaining,
            total_pause_seconds,
            status,
        );

        // Publish to subscription channel
        publish_clock_update(tournament_id, clock.clone());

        Ok(clock)
    }
}
