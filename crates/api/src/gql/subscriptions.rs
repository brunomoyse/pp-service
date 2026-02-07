use async_graphql::{Context, Result, Subscription};
use futures_util::Stream;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::gql::error::ResultExt;
use crate::gql::types::{
    PlayerRegistrationEvent, Role, SeatingChangeEvent, TournamentClock, UserNotification,
};

/// Per-tournament channels for real-time updates
struct TournamentChannels {
    registrations: broadcast::Sender<PlayerRegistrationEvent>,
    seating: broadcast::Sender<SeatingChangeEvent>,
    clock: broadcast::Sender<TournamentClock>,
}

impl TournamentChannels {
    fn new() -> Self {
        Self {
            registrations: broadcast::channel(100).0,
            seating: broadcast::channel(100).0,
            clock: broadcast::channel(100).0,
        }
    }
}

/// All subscription channels
struct SubscriptionChannels {
    /// Per-tournament channels (registrations, seating, clock)
    tournaments: HashMap<Uuid, TournamentChannels>,
    /// Per-user notification channels
    users: HashMap<Uuid, broadcast::Sender<UserNotification>>,
    /// Per-club seating channels (for managers watching all club tournaments)
    clubs: HashMap<Uuid, broadcast::Sender<SeatingChangeEvent>>,
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

    fn get_or_create_tournament(&mut self, tournament_id: Uuid) -> &TournamentChannels {
        self.tournaments
            .entry(tournament_id)
            .or_insert_with(TournamentChannels::new)
    }

    fn get_or_create_user(&mut self, user_id: Uuid) -> &broadcast::Sender<UserNotification> {
        self.users
            .entry(user_id)
            .or_insert_with(|| broadcast::channel(100).0)
    }

    fn get_or_create_club(&mut self, club_id: Uuid) -> &broadcast::Sender<SeatingChangeEvent> {
        self.clubs
            .entry(club_id)
            .or_insert_with(|| broadcast::channel(100).0)
    }
}

static CHANNELS: Lazy<Arc<Mutex<SubscriptionChannels>>> =
    Lazy::new(|| Arc::new(Mutex::new(SubscriptionChannels::new())));

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
        use crate::auth::permissions::require_role;

        // Require manager role
        let _manager = require_role(ctx, Role::Manager).await?;

        let club_uuid = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;

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
    // Send to tournament channel
    let tournament = channels.get_or_create_tournament(tournament_id);
    let _ = tournament.seating.send(event.clone());

    // Also send to club channel (for managers watching all club tournaments)
    let club_sender = channels.get_or_create_club(club_id);
    let _ = club_sender.send(event);
}

/// Publish a clock update to a tournament's channel
pub fn publish_clock_update(tournament_id: Uuid, clock: TournamentClock) {
    let mut channels = CHANNELS.lock();
    let tournament = channels.get_or_create_tournament(tournament_id);
    let _ = tournament.clock.send(clock);
}

/// Cleanup tournament channels when a tournament finishes
pub fn cleanup_tournament_channels(tournament_id: Uuid) {
    let mut channels = CHANNELS.lock();
    channels.remove_tournament(&tournament_id);
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
