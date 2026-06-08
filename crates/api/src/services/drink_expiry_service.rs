use std::time::Duration;
use tokio::time::{interval, Interval};
use tracing::{error, info};

use crate::gql::domains::drinks::service::run_expiry;
use crate::AppState;

// Run the sweep hourly. The job is idempotent and only touches wallets that have an
// expired lot, so a frequent tick is cheap; "nightly" is the intent, hourly is the
// safety margin if the process restarts.
const EXPIRY_INTERVAL_SECONDS: u64 = 3600;

/// Background job that posts negative `expiry` ledger entries for unredeemed expired
/// drink credits, keeping each wallet's cached balance equal to SUM(delta).
pub struct DrinkExpiryService {
    state: AppState,
    interval: Interval,
}

impl DrinkExpiryService {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            interval: interval(Duration::from_secs(EXPIRY_INTERVAL_SECONDS)),
        }
    }

    pub async fn run(&mut self) {
        info!("Starting drink credit expiry service");
        loop {
            self.interval.tick().await;
            match run_expiry(&self.state.db, chrono::Utc::now()).await {
                Ok(expired) if expired > 0 => {
                    info!("Expired {} unredeemed drink credit(s)", expired);
                }
                Ok(_) => {}
                Err(e) => error!("Error running drink credit expiry: {}", e),
            }
        }
    }
}

pub fn spawn_drink_expiry_service(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut service = DrinkExpiryService::new(state);
        service.run().await;
    })
}
