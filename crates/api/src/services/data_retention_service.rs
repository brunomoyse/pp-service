use std::time::Duration;

use infra::repos::{device_tokens, refresh_tokens, users};
use tokio::time::{interval, Interval};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::AppState;

// Defaults: sweep daily, anonymize player accounts dormant for ~3 years, and
// cap each run to a batch so a large backlog is worked down gradually rather
// than in one long transaction. All three are overridable via env.
const DEFAULT_INTERVAL_HOURS: u64 = 24;
const DEFAULT_RETENTION_DAYS: i32 = 1095;
const DEFAULT_BATCH_LIMIT: i64 = 200;

/// Configuration, read once at startup. The service is only spawned when
/// `ENABLE_DATA_RETENTION` is truthy (gated in `main.rs`), so this struct
/// always represents an explicitly enabled job.
pub struct RetentionConfig {
    pub retention_days: i32,
    pub batch_limit: i64,
    pub interval: Duration,
}

impl RetentionConfig {
    pub fn from_env() -> Self {
        let retention_days = env_parse("DATA_RETENTION_DAYS").unwrap_or(DEFAULT_RETENTION_DAYS);
        let batch_limit = env_parse("DATA_RETENTION_BATCH_LIMIT").unwrap_or(DEFAULT_BATCH_LIMIT);
        let interval_hours: u64 =
            env_parse("DATA_RETENTION_INTERVAL_HOURS").unwrap_or(DEFAULT_INTERVAL_HOURS);

        Self {
            retention_days: retention_days.max(1),
            batch_limit: batch_limit.clamp(1, 10_000),
            interval: Duration::from_secs(interval_hours.max(1) * 3600),
        }
    }
}

fn env_parse<T: std::str::FromStr>(key: &str) -> Option<T> {
    std::env::var(key).ok().and_then(|v| v.trim().parse().ok())
}

/// Whether the retention sweep is enabled. Defaults to OFF — anonymization is
/// destructive, so it must be deliberately turned on per environment.
pub fn is_enabled() -> bool {
    matches!(
        std::env::var("ENABLE_DATA_RETENTION")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Scheduled GDPR data-retention sweep: anonymizes player accounts with no
/// activity inside the retention window, reusing the same anonymization path as
/// self-service account deletion (scrub PII, keep the row for result integrity,
/// then revoke the account's tokens).
pub struct DataRetentionService {
    state: AppState,
    config: RetentionConfig,
    interval: Interval,
}

impl DataRetentionService {
    pub fn new(state: AppState) -> Self {
        let config = RetentionConfig::from_env();
        Self {
            interval: interval(config.interval),
            state,
            config,
        }
    }

    pub async fn run(&mut self) {
        info!(
            "Starting data-retention service (retention_days={}, batch_limit={}, interval={}s)",
            self.config.retention_days,
            self.config.batch_limit,
            self.config.interval.as_secs()
        );
        loop {
            self.interval.tick().await;
            match self.sweep().await {
                Ok(0) => {}
                Ok(n) => info!("Data retention: anonymized {n} dormant account(s)"),
                Err(e) => error!("Data retention sweep failed: {e}"),
            }
        }
    }

    /// Anonymize one batch of dormant accounts; returns how many were processed.
    async fn sweep(&self) -> Result<usize, sqlx::Error> {
        let db = &self.state.db;
        let ids = users::find_inactive_player_ids(
            db,
            self.config.retention_days,
            self.config.batch_limit,
        )
        .await?;

        let mut processed = 0usize;
        for id in ids {
            match self.anonymize_account(id).await {
                Ok(()) => processed += 1,
                // Skip-and-continue: one bad row shouldn't abort the whole sweep.
                Err(e) => warn!("Data retention: failed to anonymize {id}: {e}"),
            }
        }
        Ok(processed)
    }

    async fn anonymize_account(&self, id: Uuid) -> Result<(), sqlx::Error> {
        let db = &self.state.db;
        users::anonymize(db, id).await?;
        device_tokens::delete_all_for_user(db, id).await?;
        refresh_tokens::revoke_all_for_user(db, id).await?;
        Ok(())
    }
}

pub fn spawn_data_retention_service(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut service = DataRetentionService::new(state);
        service.run().await;
    })
}
