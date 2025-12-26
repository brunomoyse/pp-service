use chrono::Utc;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::{interval, Interval};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::gql::subscriptions::publish_clock_update;
use crate::gql::types::{ClockStatus, TournamentClock, TournamentStructure};
use crate::AppState;
use infra::repos::{ClockStatus as InfraClockStatus, TournamentClockRepo};

pub struct ClockService {
    state: AppState,
    interval: Interval,
}

impl ClockService {
    pub fn new(state: AppState) -> Self {
        // Tick every 5 seconds to check for level advances
        let interval = interval(Duration::from_secs(5));

        Self { state, interval }
    }

    /// Start the background clock service
    pub async fn run(&mut self) {
        info!("Starting tournament clock service");

        loop {
            self.interval.tick().await;

            if let Err(e) = self.process_tournaments().await {
                error!("Error processing tournament clocks: {}", e);
            }
        }
    }

    /// Process all tournaments and advance levels if needed
    async fn process_tournaments(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let repo = TournamentClockRepo::new(self.state.db.clone());

        // Get tournaments that need level advancement
        let tournament_ids = repo.get_tournaments_to_advance().await?;

        for tournament_id in tournament_ids {
            match repo.advance_level(tournament_id, true, None).await {
                Ok(clock_row) => {
                    info!("Auto-advanced level for tournament {}", tournament_id);

                    // Publish clock update to subscribers
                    if let Ok(Some(clock)) = self
                        .build_clock_update(&repo, tournament_id, &clock_row)
                        .await
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

        Ok(())
    }

    /// Build a TournamentClock from the clock row for publishing
    async fn build_clock_update(
        &self,
        repo: &TournamentClockRepo,
        tournament_id: Uuid,
        clock_row: &infra::models::TournamentClockRow,
    ) -> Result<Option<TournamentClock>, Box<dyn std::error::Error + Send + Sync>> {
        let structure = repo.get_current_structure(tournament_id).await.ok();

        // Use get_next_structure() instead of fetching all structures
        let next_structure = repo
            .get_next_structure(tournament_id, clock_row.current_level)
            .await
            .ok()
            .flatten()
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
            });

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
            current_structure: structure.as_ref().map(|s| TournamentStructure {
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
