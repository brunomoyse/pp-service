use async_graphql::{Context, Object, Result, Subscription, ID};
use chrono::Utc;
use futures_util::Stream;
use std::str::FromStr;
use std::time::Duration as StdDuration;
use tokio::time::interval;
use uuid::Uuid;

use crate::gql::types::{ClockStatus, ClockUpdate, Role, TournamentClock, TournamentStructure};
use crate::{auth::permissions::require_role, AppState};
use infra::repos::{ClockStatus as InfraClockStatus, TournamentClockRepo};

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
        let repo = TournamentClockRepo::new(state.db.clone());
        let tournament_id: Uuid = tournament_id.parse()?;

        if let Some(clock_row) = repo.get_clock(tournament_id).await? {
            let structure = repo.get_current_structure(tournament_id).await.ok();

            // Calculate time remaining
            let time_remaining =
                if let Ok(status) = InfraClockStatus::from_str(&clock_row.clock_status) {
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
                                repo.get_current_structure(tournament_id).await
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
                current_structure: structure.map(|s| TournamentStructure {
                    id: s.id.into(),
                    tournament_id: s.tournament_id.into(),
                    level_number: s.level_number,
                    small_blind: s.small_blind,
                    big_blind: s.big_blind,
                    ante: s.ante,
                    duration_minutes: s.duration_minutes,
                    is_break: s.is_break,
                    break_duration_minutes: s.break_duration_minutes,
                }),
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
        let repo = TournamentClockRepo::new(state.db.clone());
        let tournament_id: Uuid = tournament_id.parse()?;

        let structures = repo.get_all_structures(tournament_id).await?;

        Ok(structures
            .into_iter()
            .map(|s| TournamentStructure {
                id: s.id.into(),
                tournament_id: s.tournament_id.into(),
                level_number: s.level_number,
                small_blind: s.small_blind,
                big_blind: s.big_blind,
                ante: s.ante,
                duration_minutes: s.duration_minutes,
                is_break: s.is_break,
                break_duration_minutes: s.break_duration_minutes,
            })
            .collect())
    }
}

pub struct TournamentClockMutation;

#[Object]
impl TournamentClockMutation {
    /// Initialize tournament clock
    pub async fn create_tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<TournamentClock> {
        let _manager = require_role(ctx, Role::Manager).await?;
        let state = ctx.data::<AppState>()?;
        let repo = TournamentClockRepo::new(state.db.clone());
        let tournament_id: Uuid = tournament_id.parse()?;

        let clock_row = repo.create_clock(tournament_id).await?;
        let structure = repo.get_current_structure(tournament_id).await.ok();

        // Show full duration of first level when clock is created (stopped state)
        let time_remaining_seconds = structure.as_ref().map(|s| (s.duration_minutes as i64) * 60);

        Ok(TournamentClock {
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
            current_structure: structure.map(|s| TournamentStructure {
                id: s.id.into(),
                tournament_id: s.tournament_id.into(),
                level_number: s.level_number,
                small_blind: s.small_blind,
                big_blind: s.big_blind,
                ante: s.ante,
                duration_minutes: s.duration_minutes,
                is_break: s.is_break,
                break_duration_minutes: s.break_duration_minutes,
            }),
        })
    }

    /// Start tournament clock
    pub async fn start_tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<TournamentClock> {
        let manager = require_role(ctx, Role::Manager).await?;
        let state = ctx.data::<AppState>()?;
        let repo = TournamentClockRepo::new(state.db.clone());
        let tournament_id: Uuid = tournament_id.parse()?;

        let clock_row = repo
            .start_clock(tournament_id, Some(manager.id.parse()?))
            .await?;
        let structure = repo.get_current_structure(tournament_id).await.ok();

        // Calculate time remaining
        let time_remaining = if let Some(end_time) = clock_row.level_end_time {
            let remaining = end_time - Utc::now();
            Some(remaining.num_seconds().max(0))
        } else {
            None
        };

        Ok(TournamentClock {
            id: clock_row.id.into(),
            tournament_id: clock_row.tournament_id.into(),
            status: ClockStatus::Running,
            current_level: clock_row.current_level,
            time_remaining_seconds: time_remaining,
            level_started_at: clock_row.level_started_at,
            level_end_time: clock_row.level_end_time,
            total_pause_duration_seconds: 0,
            auto_advance: clock_row.auto_advance,
            current_structure: structure.map(|s| TournamentStructure {
                id: s.id.into(),
                tournament_id: s.tournament_id.into(),
                level_number: s.level_number,
                small_blind: s.small_blind,
                big_blind: s.big_blind,
                ante: s.ante,
                duration_minutes: s.duration_minutes,
                is_break: s.is_break,
                break_duration_minutes: s.break_duration_minutes,
            }),
        })
    }

    /// Pause tournament clock
    pub async fn pause_tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<TournamentClock> {
        let manager = require_role(ctx, Role::Manager).await?;
        let state = ctx.data::<AppState>()?;
        let repo = TournamentClockRepo::new(state.db.clone());
        let tournament_id: Uuid = tournament_id.parse()?;

        let clock_row = repo
            .pause_clock(tournament_id, Some(manager.id.parse()?))
            .await?;
        let structure = repo.get_current_structure(tournament_id).await.ok();

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

        Ok(TournamentClock {
            id: clock_row.id.into(),
            tournament_id: clock_row.tournament_id.into(),
            status: ClockStatus::Paused,
            current_level: clock_row.current_level,
            time_remaining_seconds: time_remaining,
            level_started_at: clock_row.level_started_at,
            level_end_time: clock_row.level_end_time,
            total_pause_duration_seconds: total_pause_seconds,
            auto_advance: clock_row.auto_advance,
            current_structure: structure.map(|s| TournamentStructure {
                id: s.id.into(),
                tournament_id: s.tournament_id.into(),
                level_number: s.level_number,
                small_blind: s.small_blind,
                big_blind: s.big_blind,
                ante: s.ante,
                duration_minutes: s.duration_minutes,
                is_break: s.is_break,
                break_duration_minutes: s.break_duration_minutes,
            }),
        })
    }

    /// Resume tournament clock
    pub async fn resume_tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<TournamentClock> {
        let manager = require_role(ctx, Role::Manager).await?;
        let state = ctx.data::<AppState>()?;
        let repo = TournamentClockRepo::new(state.db.clone());
        let tournament_id: Uuid = tournament_id.parse()?;

        let clock_row = repo
            .resume_clock(tournament_id, Some(manager.id.parse()?))
            .await?;
        let structure = repo.get_current_structure(tournament_id).await.ok();

        // Calculate time remaining
        let time_remaining = if let Some(end_time) = clock_row.level_end_time {
            let remaining = end_time - Utc::now();
            Some(remaining.num_seconds().max(0))
        } else {
            None
        };

        let total_pause_seconds = clock_row.total_pause_duration.microseconds / 1_000_000;

        Ok(TournamentClock {
            id: clock_row.id.into(),
            tournament_id: clock_row.tournament_id.into(),
            status: ClockStatus::Running,
            current_level: clock_row.current_level,
            time_remaining_seconds: time_remaining,
            level_started_at: clock_row.level_started_at,
            level_end_time: clock_row.level_end_time,
            total_pause_duration_seconds: total_pause_seconds,
            auto_advance: clock_row.auto_advance,
            current_structure: structure.map(|s| TournamentStructure {
                id: s.id.into(),
                tournament_id: s.tournament_id.into(),
                level_number: s.level_number,
                small_blind: s.small_blind,
                big_blind: s.big_blind,
                ante: s.ante,
                duration_minutes: s.duration_minutes,
                is_break: s.is_break,
                break_duration_minutes: s.break_duration_minutes,
            }),
        })
    }

    /// Manually advance to next level
    pub async fn advance_tournament_level(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<TournamentClock> {
        let manager = require_role(ctx, Role::Manager).await?;
        let state = ctx.data::<AppState>()?;
        let repo = TournamentClockRepo::new(state.db.clone());
        let tournament_id: Uuid = tournament_id.parse()?;

        let clock_row = repo
            .advance_level(tournament_id, false, Some(manager.id.parse()?))
            .await?;
        let structure = repo.get_current_structure(tournament_id).await.ok();

        // Calculate time remaining for new level
        let time_remaining = if let Some(end_time) = clock_row.level_end_time {
            let remaining = end_time - Utc::now();
            Some(remaining.num_seconds().max(0))
        } else {
            None
        };

        Ok(TournamentClock {
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
            total_pause_duration_seconds: 0, // Reset for new level
            auto_advance: clock_row.auto_advance,
            current_structure: structure.map(|s| TournamentStructure {
                id: s.id.into(),
                tournament_id: s.tournament_id.into(),
                level_number: s.level_number,
                small_blind: s.small_blind,
                big_blind: s.big_blind,
                ante: s.ante,
                duration_minutes: s.duration_minutes,
                is_break: s.is_break,
                break_duration_minutes: s.break_duration_minutes,
            }),
        })
    }

    /// Manually revert to previous level
    pub async fn revert_tournament_level(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<TournamentClock> {
        let manager = require_role(ctx, Role::Manager).await?;
        let state = ctx.data::<AppState>()?;
        let repo = TournamentClockRepo::new(state.db.clone());
        let tournament_id: Uuid = tournament_id.parse()?;

        let clock_row = repo
            .revert_level(tournament_id, Some(manager.id.parse()?))
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => {
                    async_graphql::Error::new("Cannot revert: Tournament is already at level 1")
                }
                _ => async_graphql::Error::new(format!("Failed to revert level: {}", e)),
            })?;
        let structure = repo.get_current_structure(tournament_id).await.ok();

        // Calculate time remaining for reverted level
        let time_remaining = if let Some(end_time) = clock_row.level_end_time {
            let remaining = end_time - Utc::now();
            Some(remaining.num_seconds().max(0))
        } else {
            None
        };

        Ok(TournamentClock {
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
            total_pause_duration_seconds: 0, // Reset for reverted level
            auto_advance: clock_row.auto_advance,
            current_structure: structure.map(|s| TournamentStructure {
                id: s.id.into(),
                tournament_id: s.tournament_id.into(),
                level_number: s.level_number,
                small_blind: s.small_blind,
                big_blind: s.big_blind,
                ante: s.ante,
                duration_minutes: s.duration_minutes,
                is_break: s.is_break,
                break_duration_minutes: s.break_duration_minutes,
            }),
        })
    }
}

pub struct TournamentClockSubscription;

#[Subscription]
impl TournamentClockSubscription {
    /// Subscribe to tournament clock updates
    pub async fn tournament_clock_updates(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
    ) -> Result<impl Stream<Item = ClockUpdate>> {
        let state = ctx.data::<AppState>()?;
        let repo = TournamentClockRepo::new(state.db.clone());
        let tournament_id: Uuid = tournament_id.parse()?;

        // Create a stream that emits every second
        let mut interval = interval(StdDuration::from_secs(1));

        Ok(async_stream::stream! {
            loop {
                interval.tick().await;

                // Get current clock state
                if let Ok(Some(clock_row)) = repo.get_clock(tournament_id).await {
                    if let Ok(structure) = repo.get_current_structure(tournament_id).await {
                        let clock_status = InfraClockStatus::from_str(&clock_row.clock_status).ok()
                            .unwrap_or(InfraClockStatus::Stopped);

                        // Calculate time remaining
                        let time_remaining = match clock_status {
                            InfraClockStatus::Running => {
                                if let Some(end_time) = clock_row.level_end_time {
                                    let remaining = end_time - Utc::now();
                                    Some(remaining.num_seconds().max(0))
                                } else {
                                    None
                                }
                            },
                            InfraClockStatus::Paused => {
                                if let (Some(end_time), Some(pause_start)) = (clock_row.level_end_time, clock_row.pause_started_at) {
                                    let remaining = end_time - pause_start;
                                    Some(remaining.num_seconds().max(0))
                                } else {
                                    None
                                }
                            },
                            InfraClockStatus::Stopped => {
                                // Show full duration of current level when stopped
                                // If we can't get current level, try to get level 1 as fallback
                                if let Ok(all_structures) = repo.get_all_structures(tournament_id).await {
                                    if let Some(current_struct) = all_structures.iter().find(|s| s.level_number == clock_row.current_level) {
                                        Some((current_struct.duration_minutes as i64) * 60)
                                    } else if let Some(first_struct) = all_structures.iter().find(|s| s.level_number == 1) {
                                        Some((first_struct.duration_minutes as i64) * 60)
                                    } else {
                                        Some((structure.duration_minutes as i64) * 60)
                                    }
                                } else {
                                    Some((structure.duration_minutes as i64) * 60)
                                }
                            }
                        };

                        // Get next level preview
                        let next_level_preview = repo.get_all_structures(tournament_id).await
                            .ok()
                            .and_then(|structures| {
                                structures.into_iter()
                                    .find(|s| s.level_number == clock_row.current_level + 1)
                                    .map(|s| TournamentStructure {
                                        id: s.id.into(),
                                        tournament_id: s.tournament_id.into(),
                                        level_number: s.level_number,
                                        small_blind: s.small_blind,
                                        big_blind: s.big_blind,
                                        ante: s.ante,
                                        duration_minutes: s.duration_minutes,
                                        is_break: s.is_break,
                                        break_duration_minutes: s.break_duration_minutes,
                                    })
                            });

                        let update = ClockUpdate {
                            tournament_id: tournament_id.into(),
                            status: clock_status.into(),
                            current_level: clock_row.current_level,
                            time_remaining_seconds: time_remaining,
                            small_blind: structure.small_blind,
                            big_blind: structure.big_blind,
                            ante: structure.ante,
                            is_break: structure.is_break,
                            level_duration_minutes: structure.duration_minutes,
                            next_level_preview,
                        };

                        yield update;
                    }
                }
            }
        })
    }
}
