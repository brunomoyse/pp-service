use async_graphql::ID;
use chrono::Utc;
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::{interval, Interval};
use tracing::{error, info};
use uuid::Uuid;

use crate::gql::subscriptions::publish_user_notification;
use crate::gql::types::{NotificationType, UserNotification};
use crate::AppState;
use infra::repos::{TournamentRegistrationRepo, TournamentRepo};

const NOTIFICATION_WINDOW_MINUTES: i32 = 16; // Check for tournaments starting within 16 minutes
const CHECK_INTERVAL_SECONDS: u64 = 60; // Check every minute

pub struct NotificationService {
    state: AppState,
    interval: Interval,
    notified_tournaments: HashSet<Uuid>, // Track which tournaments we've already notified
}

impl NotificationService {
    pub fn new(state: AppState) -> Self {
        let interval = interval(Duration::from_secs(CHECK_INTERVAL_SECONDS));

        Self {
            state,
            interval,
            notified_tournaments: HashSet::new(),
        }
    }

    /// Start the background notification service
    pub async fn run(&mut self) {
        info!("Starting notification service");

        loop {
            self.interval.tick().await;

            if let Err(e) = self.check_upcoming_tournaments().await {
                error!("Error checking upcoming tournaments: {}", e);
            }
        }
    }

    /// Check for tournaments starting soon and notify registered players
    async fn check_upcoming_tournaments(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tournament_repo = TournamentRepo::new(self.state.db.clone());
        let registration_repo = TournamentRegistrationRepo::new(self.state.db.clone());

        // Get tournaments starting within the next 16 minutes
        let upcoming = tournament_repo
            .get_tournaments_starting_soon(NOTIFICATION_WINDOW_MINUTES)
            .await?;

        for tournament in upcoming {
            // Skip if we've already notified for this tournament
            if self.notified_tournaments.contains(&tournament.id) {
                continue;
            }

            // Get all registered players for this tournament
            let registrations = registration_repo.get_by_tournament(tournament.id).await?;

            info!(
                "Sending 'starting soon' notifications for tournament {} to {} players",
                tournament.name,
                registrations.len()
            );

            // Send notification to each registered player
            for registration in registrations {
                let notification = UserNotification {
                    id: ID::from(Uuid::new_v4().to_string()),
                    user_id: ID::from(registration.user_id.to_string()),
                    notification_type: NotificationType::TournamentStartingSoon,
                    title: "Tournament Starting Soon".to_string(),
                    message: format!(
                        "{} is starting in about 15 minutes",
                        tournament.name
                    ),
                    tournament_id: Some(ID::from(tournament.id.to_string())),
                    created_at: Utc::now(),
                };

                publish_user_notification(notification);
            }

            // Mark this tournament as notified
            self.notified_tournaments.insert(tournament.id);
        }

        // Clean up old entries (tournaments that have already passed)
        // This prevents memory from growing indefinitely
        self.cleanup_old_entries().await;

        Ok(())
    }

    /// Remove tournaments from the notified set if they've already started
    async fn cleanup_old_entries(&mut self) {
        let tournament_repo = TournamentRepo::new(self.state.db.clone());

        // Keep only tournaments that still exist and haven't started
        let mut to_remove = Vec::new();

        for tournament_id in &self.notified_tournaments {
            match tournament_repo.get(*tournament_id).await {
                Ok(Some(tournament)) => {
                    // Remove if tournament has already started (start_time is in the past)
                    if tournament.start_time < Utc::now() {
                        to_remove.push(*tournament_id);
                    }
                }
                Ok(None) => {
                    // Tournament was deleted
                    to_remove.push(*tournament_id);
                }
                Err(_) => {
                    // Keep it in case of error, we'll try again later
                }
            }
        }

        for id in to_remove {
            self.notified_tournaments.remove(&id);
        }
    }
}

/// Spawn the notification service as a background task
pub fn spawn_notification_service(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut service = NotificationService::new(state);
        service.run().await;
    })
}
