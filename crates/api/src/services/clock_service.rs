use chrono::Utc;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::{interval, Interval};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::gql::subscriptions::{cleanup_inactive_channels, publish_clock_update};
use crate::gql::types::{ClockStatus, TournamentClock, TournamentStructure};
use crate::AppState;
use infra::repos::{
    tournament_clock, tournament_clock::ClockStatus as InfraClockStatus, tournaments,
};

// Check for stale tournaments every 60 ticks (5 minutes at 5 second intervals)
const STALE_CHECK_INTERVAL: u64 = 60;
// Auto-finish tournaments that have been running for more than 24 hours
const STALE_TOURNAMENT_HOURS: i32 = 24;
// Remove subscription channels inactive for more than 2 hours
const INACTIVE_CHANNEL_HOURS: i64 = 2;

pub struct ClockService {
    state: AppState,
    interval: Interval,
    tick_count: u64,
}

impl ClockService {
    pub fn new(state: AppState) -> Self {
        // Tick every 5 seconds to check for level advances
        let interval = interval(Duration::from_secs(5));

        Self {
            state,
            interval,
            tick_count: 0,
        }
    }

    /// Start the background clock service
    pub async fn run(&mut self) {
        info!("Starting tournament clock service");

        loop {
            self.interval.tick().await;
            self.tick_count += 1;

            if let Err(e) = self.process_tournaments().await {
                error!("Error processing tournament clocks: {}", e);
            }

            // Check for stale tournaments every STALE_CHECK_INTERVAL ticks
            if self.tick_count.is_multiple_of(STALE_CHECK_INTERVAL) {
                if let Err(e) = self.process_stale_tournaments().await {
                    error!("Error processing stale tournaments: {}", e);
                }

                // Clean up expired refresh tokens
                if let Err(e) = infra::repos::refresh_tokens::delete_expired(&self.state.db).await {
                    error!("Error cleaning up expired refresh tokens: {}", e);
                }

                // Clean up inactive subscription channels to prevent memory leaks
                cleanup_inactive_channels(INACTIVE_CHANNEL_HOURS);
            }
        }
    }

    /// Process all tournaments and advance levels if needed
    async fn process_tournaments(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get tournaments that need level advancement
        let tournament_ids = tournament_clock::get_tournaments_to_advance(&self.state.db).await?;

        for tournament_id in tournament_ids {
            match tournament_clock::advance_level(&self.state.db, tournament_id, true, None).await {
                Ok(clock_row) => {
                    info!("Auto-advanced level for tournament {}", tournament_id);

                    // Publish clock update to subscribers
                    if let Ok(Some(clock)) =
                        self.build_clock_update(tournament_id, &clock_row).await
                    {
                        publish_clock_update(tournament_id, clock);
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to auto-advance level for tournament {}: {}",
                        tournament_id, e
                    );
                }
            }
        }

        // Handle tournaments at their final level - stop the clock
        let final_level_tournaments =
            tournament_clock::get_tournaments_at_final_level(&self.state.db).await?;

        for tournament_id in final_level_tournaments {
            match tournament_clock::stop_clock_final_level(&self.state.db, tournament_id).await {
                Ok(clock_row) => {
                    info!(
                        "Stopped clock for tournament {} - final level complete",
                        tournament_id
                    );

                    // Publish clock update to subscribers
                    if let Ok(Some(clock)) =
                        self.build_clock_update(tournament_id, &clock_row).await
                    {
                        publish_clock_update(tournament_id, clock);
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to stop clock for tournament {} at final level: {}",
                        tournament_id, e
                    );
                }
            }
        }

        Ok(())
    }

    /// Check for and auto-finish stale tournaments (running > 24 hours)
    async fn process_stale_tournaments(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stale_tournaments =
            tournaments::list_stale(&self.state.db, STALE_TOURNAMENT_HOURS).await?;

        for tournament in stale_tournaments {
            match tournaments::auto_finish(&self.state.db, tournament.id).await {
                Ok(Some(finished)) => {
                    warn!(
                        "Auto-finished stale tournament '{}' (ID: {}) - running for over {} hours",
                        finished.name, finished.id, STALE_TOURNAMENT_HOURS
                    );
                }
                Ok(None) => {
                    warn!(
                        "Failed to auto-finish tournament {} - may have already been finished",
                        tournament.id
                    );
                }
                Err(e) => {
                    error!(
                        "Error auto-finishing stale tournament {}: {}",
                        tournament.id, e
                    );
                }
            }
        }

        Ok(())
    }

    /// Build a TournamentClock from the clock row for publishing
    async fn build_clock_update(
        &self,
        tournament_id: Uuid,
        clock_row: &infra::models::TournamentClockRow,
    ) -> Result<Option<TournamentClock>, Box<dyn std::error::Error + Send + Sync>> {
        let structure = tournament_clock::get_current_structure(&self.state.db, tournament_id)
            .await
            .ok();

        // Use get_next_structure() instead of fetching all structures
        let next_structure = tournament_clock::get_next_structure(
            &self.state.db,
            tournament_id,
            clock_row.current_level,
        )
        .await
        .ok()
        .flatten()
        .map(TournamentStructure::from);

        let clock_status = InfraClockStatus::from_str(&clock_row.clock_status)
            .ok()
            .unwrap_or(InfraClockStatus::Stopped);

        let time_remaining = match clock_status {
            InfraClockStatus::Running => clock_row
                .level_end_time
                .map(|end_time| (end_time - Utc::now()).num_seconds().max(0)),
            InfraClockStatus::Paused => {
                if let (Some(end_time), Some(pause_start)) =
                    (clock_row.level_end_time, clock_row.pause_started_at)
                {
                    Some((end_time - pause_start).num_seconds().max(0))
                } else {
                    None
                }
            }
            InfraClockStatus::Stopped => {
                structure.as_ref().map(|s| (s.duration_minutes as i64) * 60)
            }
        };

        let total_pause_seconds = clock_row.total_pause_duration.microseconds / 1_000_000;

        let status: ClockStatus = clock_status.into();

        Ok(Some(TournamentClock {
            id: clock_row.id.into(),
            tournament_id: clock_row.tournament_id.into(),
            status,
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
            small_blind: structure.as_ref().map(|s| s.small_blind),
            big_blind: structure.as_ref().map(|s| s.big_blind),
            ante: structure.as_ref().map(|s| s.ante),
            is_break: structure.as_ref().map(|s| s.is_break),
            level_duration_minutes: structure.as_ref().map(|s| s.duration_minutes),
        }))
    }
}

/// Spawn the clock service as a background task
pub fn spawn_clock_service(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut service = ClockService::new(state);
        service.run().await;
    })
}
