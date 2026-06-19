use std::time::Duration;
use tokio::time::{interval, Interval};
use tracing::{error, info};

use crate::AppState;

// Hourly is plenty: a trial lapsing an hour late is harmless, and the sweep is
// idempotent + cheap (only touches paid clubs whose expiry has already passed).
const SWEEP_INTERVAL_SECONDS: u64 = 3600;

/// Background job that downgrades paid clubs back to free once their
/// subscription / trial window (`subscription_expires_at`) has lapsed. This is
/// what makes a redemption-code trial actually end without manual intervention.
pub struct SubscriptionExpiryService {
    state: AppState,
    interval: Interval,
}

impl SubscriptionExpiryService {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            interval: interval(Duration::from_secs(SWEEP_INTERVAL_SECONDS)),
        }
    }

    pub async fn run(&mut self) {
        info!("Starting subscription expiry service");
        loop {
            self.interval.tick().await;
            match infra::repos::clubs::downgrade_expired(&self.state.db, chrono::Utc::now()).await {
                Ok(downgraded) if downgraded > 0 => {
                    info!(
                        "Downgraded {} club(s) with a lapsed subscription",
                        downgraded
                    );
                }
                Ok(_) => {}
                Err(e) => error!("Error running subscription expiry sweep: {}", e),
            }
        }
    }
}

pub fn spawn_subscription_expiry_service(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut service = SubscriptionExpiryService::new(state);
        service.run().await;
    })
}
