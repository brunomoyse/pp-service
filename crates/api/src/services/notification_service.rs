use async_graphql::ID;
use chrono::Utc;
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::{interval, Interval};
use tracing::{error, info};
use uuid::Uuid;

use crate::gql::subscriptions::publish_user_notification;
use crate::gql::types::{NotificationType, UserNotification, TITLE_TOURNAMENT_STARTING};
use crate::AppState;
use infra::repos::{tournament_registrations, tournaments, users};

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
    async fn check_upcoming_tournaments(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get tournaments starting within the next 16 minutes
        let upcoming =
            tournaments::list_starting_soon(&self.state.db, NOTIFICATION_WINDOW_MINUTES).await?;

        for tournament in upcoming {
            // Skip if we've already notified for this tournament
            if self.notified_tournaments.contains(&tournament.id) {
                continue;
            }

            // Get all registered players for this tournament
            let registrations =
                tournament_registrations::list_by_tournament(&self.state.db, tournament.id).await?;

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
                    title: TITLE_TOURNAMENT_STARTING.to_string(),
                    message: format!("{} is starting in about 15 minutes", tournament.name),
                    tournament_id: Some(ID::from(tournament.id.to_string())),
                    created_at: Utc::now(),
                };

                publish_user_notification(notification);

                // Send email notification (fire-and-forget)
                if let Some(email_service) = self.state.email_service() {
                    if let Ok(Some(user_row)) =
                        users::get_by_id(&self.state.db, registration.user_id).await
                    {
                        let locale = super::email_service::Locale::from_str_lossy(&user_row.locale);
                        super::email_service::spawn_email(
                            email_service.clone(),
                            user_row.email,
                            user_row.first_name,
                            super::email_service::EmailType::TournamentStartingSoon {
                                tournament_name: tournament.name.clone(),
                                locale,
                            },
                        );
                    }
                }
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
        // Collect all tournament IDs to check
        let ids: Vec<Uuid> = self.notified_tournaments.iter().copied().collect();

        if ids.is_empty() {
            return;
        }

        // Bulk fetch all tournaments in one query instead of N queries
        let tournaments = match tournaments::get_by_ids(&self.state.db, &ids).await {
            Ok(t) => t,
            Err(_) => return, // Keep entries on error, retry later
        };

        // Create set of valid tournament IDs that are still upcoming
        let now = Utc::now();
        let still_upcoming: HashSet<Uuid> = tournaments
            .into_iter()
            .filter(|t| t.start_time >= now)
            .map(|t| t.id)
            .collect();

        // Find IDs to remove (not in still_upcoming means either started or deleted)
        let to_remove: Vec<Uuid> = self
            .notified_tournaments
            .iter()
            .filter(|id| !still_upcoming.contains(id))
            .copied()
            .collect();

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
