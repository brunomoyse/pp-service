use std::time::Duration;
use tokio::time::{interval, Interval};
use tracing::{error, info, warn};

use crate::AppState;
use infra::repos::TournamentClockRepo;

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
                Ok(_) => {
                    info!("Auto-advanced level for tournament {}", tournament_id);
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
}

/// Spawn the clock service as a background task
pub fn spawn_clock_service(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut service = ClockService::new(state);
        service.run().await;
    })
}
