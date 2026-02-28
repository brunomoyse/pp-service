use async_graphql::{Context, Result, Subscription};
use chrono::{DateTime, Utc};
use futures_util::Stream;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::broadcast;
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::gql::error::ResultExt;
use crate::gql::types::{
    ActivityLogEntry, PlayerRegistrationEvent, SeatingChangeEvent, TournamentClock,
    UserNotification,
};

/// Per-tournament channels for real-time updates
struct TournamentChannels {
    registrations: broadcast::Sender<PlayerRegistrationEvent>,
    seating: broadcast::Sender<SeatingChangeEvent>,
    clock: broadcast::Sender<TournamentClock>,
    activity: broadcast::Sender<ActivityLogEntry>,
    last_activity: DateTime<Utc>,
}

impl TournamentChannels {
    fn new() -> Self {
        Self {
            registrations: broadcast::channel(100).0,
            seating: broadcast::channel(100).0,
            clock: broadcast::channel(100).0,
            activity: broadcast::channel(100).0,
            last_activity: Utc::now(),
        }
    }

    fn update_activity(&mut self) {
        self.last_activity = Utc::now();
    }
}

/// Wrapper for broadcast sender with activity tracking
struct ActivityTrackedSender<T: Clone> {
    sender: broadcast::Sender<T>,
    last_activity: DateTime<Utc>,
}

impl<T: Clone> ActivityTrackedSender<T> {
    fn new(capacity: usize) -> Self {
        Self {
            sender: broadcast::channel(capacity).0,
            last_activity: Utc::now(),
        }
    }

    fn update_activity(&mut self) {
        self.last_activity = Utc::now();
    }
}

/// All subscription channels
struct SubscriptionChannels {
    /// Per-tournament channels (registrations, seating, clock)
    tournaments: HashMap<Uuid, TournamentChannels>,
    /// Per-user notification channels
    users: HashMap<Uuid, ActivityTrackedSender<UserNotification>>,
    /// Per-club seating channels (for managers watching all club tournaments)
    clubs: HashMap<Uuid, ActivityTrackedSender<SeatingChangeEvent>>,
}

impl SubscriptionChannels {
    fn new() -> Self {
        Self {
            tournaments: HashMap::new(),
            users: HashMap::new(),
            clubs: HashMap::new(),
        }
    }

    fn remove_tournament(&mut self, tournament_id: &Uuid) {
        self.tournaments.remove(tournament_id);
    }

    fn get_or_create_tournament(&mut self, tournament_id: Uuid) -> &mut TournamentChannels {
        let channels = self
            .tournaments
            .entry(tournament_id)
            .or_insert_with(TournamentChannels::new);
        channels.update_activity();
        channels
    }

    fn get_or_create_user(&mut self, user_id: Uuid) -> &broadcast::Sender<UserNotification> {
        let tracked = self
            .users
            .entry(user_id)
            .or_insert_with(|| ActivityTrackedSender::new(100));
        tracked.update_activity();
        &tracked.sender
    }

    fn get_or_create_club(&mut self, club_id: Uuid) -> &broadcast::Sender<SeatingChangeEvent> {
        let tracked = self
            .clubs
            .entry(club_id)
            .or_insert_with(|| ActivityTrackedSender::new(100));
        tracked.update_activity();
        &tracked.sender
    }

    /// Remove inactive channels (no activity for more than the specified duration)
    fn cleanup_inactive_channels(&mut self, inactive_duration_hours: i64) {
        let cutoff_time = Utc::now() - chrono::Duration::hours(inactive_duration_hours);

        // Clean up tournament channels
        let initial_tournament_count = self.tournaments.len();
        self.tournaments
            .retain(|_, channels| channels.last_activity > cutoff_time);
        let removed_tournaments = initial_tournament_count - self.tournaments.len();

        // Clean up user channels
        let initial_user_count = self.users.len();
        self.users
            .retain(|_, tracked| tracked.last_activity > cutoff_time);
        let removed_users = initial_user_count - self.users.len();

        // Clean up club channels
        let initial_club_count = self.clubs.len();
        self.clubs
            .retain(|_, tracked| tracked.last_activity > cutoff_time);
        let removed_clubs = initial_club_count - self.clubs.len();

        if removed_tournaments > 0 || removed_users > 0 || removed_clubs > 0 {
            tracing::info!(
                "Cleaned up inactive channels: {} tournaments, {} users, {} clubs",
                removed_tournaments,
                removed_users,
                removed_clubs
            );
        }
    }
}

static CHANNELS: LazyLock<Arc<Mutex<SubscriptionChannels>>> =
    LazyLock::new(|| Arc::new(Mutex::new(SubscriptionChannels::new())));

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Subscribe to tournament clock updates for a specific tournament
    async fn tournament_clock_updates(
        &self,
        tournament_id: async_graphql::ID,
    ) -> Result<impl Stream<Item = Result<TournamentClock, BroadcastStreamRecvError>>> {
        let tournament_uuid =
            Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let receiver = {
            let mut channels = CHANNELS.lock();
            let tournament = channels.get_or_create_tournament(tournament_uuid);
            tournament.clock.subscribe()
        };

        Ok(BroadcastStream::new(receiver))
    }

    /// Subscribe to player registration events for a specific tournament
    async fn tournament_registrations(
        &self,
        tournament_id: async_graphql::ID,
    ) -> Result<impl Stream<Item = Result<PlayerRegistrationEvent, BroadcastStreamRecvError>>> {
        let tournament_uuid =
            Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let receiver = {
            let mut channels = CHANNELS.lock();
            let tournament = channels.get_or_create_tournament(tournament_uuid);
            tournament.registrations.subscribe()
        };

        Ok(BroadcastStream::new(receiver))
    }

    /// Subscribe to seating changes for a specific tournament
    async fn tournament_seating_changes(
        &self,
        tournament_id: async_graphql::ID,
    ) -> Result<impl Stream<Item = Result<SeatingChangeEvent, BroadcastStreamRecvError>>> {
        let tournament_uuid =
            Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let receiver = {
            let mut channels = CHANNELS.lock();
            let tournament = channels.get_or_create_tournament(tournament_uuid);
            tournament.seating.subscribe()
        };

        Ok(BroadcastStream::new(receiver))
    }

    /// Subscribe to seating changes for all tournaments in a club (managers only)
    async fn club_seating_changes(
        &self,
        ctx: &Context<'_>,
        club_id: async_graphql::ID,
    ) -> Result<impl Stream<Item = Result<SeatingChangeEvent, BroadcastStreamRecvError>>> {
        use crate::auth::permissions::require_club_manager;

        let club_uuid = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;

        // Require manager of this specific club
        let _manager = require_club_manager(ctx, club_uuid).await?;

        let receiver = {
            let mut channels = CHANNELS.lock();
            let club_sender = channels.get_or_create_club(club_uuid);
            club_sender.subscribe()
        };

        Ok(BroadcastStream::new(receiver))
    }

    /// Subscribe to user-specific notifications (requires authentication)
    async fn user_notifications(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl Stream<Item = Result<UserNotification, BroadcastStreamRecvError>>> {
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let receiver = {
            let mut channels = CHANNELS.lock();
            let user_sender = channels.get_or_create_user(user_id);
            user_sender.subscribe()
        };

        Ok(BroadcastStream::new(receiver))
    }

    /// Subscribe to tournament activity log entries in real time
    async fn tournament_activity(
        &self,
        tournament_id: async_graphql::ID,
    ) -> Result<impl Stream<Item = Result<ActivityLogEntry, BroadcastStreamRecvError>>> {
        let tournament_uuid =
            Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let receiver = {
            let mut channels = CHANNELS.lock();
            let tournament = channels.get_or_create_tournament(tournament_uuid);
            tournament.activity.subscribe()
        };

        Ok(BroadcastStream::new(receiver))
    }
}

// ============================================================================
// Publish functions - send events to specific channels
// ============================================================================

/// Publish a registration event to a tournament's channel
pub fn publish_registration_event(event: PlayerRegistrationEvent) {
    let tournament_id = match Uuid::parse_str(event.tournament_id.as_str()) {
        Ok(id) => id,
        Err(_) => return,
    };

    let mut channels = CHANNELS.lock();
    let tournament = channels.get_or_create_tournament(tournament_id);
    // Activity is already updated by get_or_create_tournament
    let _ = tournament.registrations.send(event);
}

/// Publish a seating event to a tournament's channel and the club's channel
pub fn publish_seating_event(event: SeatingChangeEvent) {
    let tournament_id = match Uuid::parse_str(event.tournament_id.as_str()) {
        Ok(id) => id,
        Err(_) => return,
    };

    let club_id = match Uuid::parse_str(event.club_id.as_str()) {
        Ok(id) => id,
        Err(_) => return,
    };

    let mut channels = CHANNELS.lock();
    // Send to tournament channel (activity is updated by get_or_create_tournament)
    let tournament = channels.get_or_create_tournament(tournament_id);
    let _ = tournament.seating.send(event.clone());

    // Also send to club channel (activity is updated by get_or_create_club)
    let club_sender = channels.get_or_create_club(club_id);
    let _ = club_sender.send(event);
}

/// Publish a clock update to a tournament's channel
pub fn publish_clock_update(tournament_id: Uuid, clock: TournamentClock) {
    let mut channels = CHANNELS.lock();
    let tournament = channels.get_or_create_tournament(tournament_id);
    // Activity is already updated by get_or_create_tournament
    let _ = tournament.clock.send(clock);
}

/// Cleanup tournament channels when a tournament finishes
pub fn cleanup_tournament_channels(tournament_id: Uuid) {
    let mut channels = CHANNELS.lock();
    channels.remove_tournament(&tournament_id);
}

/// Cleanup inactive channels across all subscription types
/// Removes channels that haven't had activity for the specified duration
pub fn cleanup_inactive_channels(inactive_duration_hours: i64) {
    let mut channels = CHANNELS.lock();
    channels.cleanup_inactive_channels(inactive_duration_hours);
}

/// Publish a notification to a specific user's channel
pub fn publish_user_notification(notification: UserNotification) {
    let user_id = match Uuid::parse_str(notification.user_id.as_str()) {
        Ok(id) => id,
        Err(_) => return,
    };

    let mut channels = CHANNELS.lock();
    let user_sender = channels.get_or_create_user(user_id);
    let _ = user_sender.send(notification);
}

/// Publish an activity log entry to a tournament's activity channel
pub fn publish_activity_event(tournament_id: Uuid, entry: ActivityLogEntry) {
    let mut channels = CHANNELS.lock();
    let tournament = channels.get_or_create_tournament(tournament_id);
    let _ = tournament.activity.send(entry);
}
