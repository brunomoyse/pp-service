use async_graphql::{Context, Result, Subscription};
use futures_util::Stream;
use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};

use crate::gql::types::{PlayerRegistrationEvent, SeatingChangeEvent};

static REGISTRATION_BROADCASTER: Lazy<Arc<Mutex<broadcast::Sender<PlayerRegistrationEvent>>>> =
    Lazy::new(|| {
        let (tx, _) = broadcast::channel(1000);
        Arc::new(Mutex::new(tx))
    });

static SEATING_BROADCASTER: Lazy<Arc<Mutex<broadcast::Sender<SeatingChangeEvent>>>> =
    Lazy::new(|| {
        let (tx, _) = broadcast::channel(1000);
        Arc::new(Mutex::new(tx))
    });

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Subscribe to tournament clock updates
    async fn tournament_clock_updates(
        &self,
        ctx: &Context<'_>,
        tournament_id: async_graphql::ID,
    ) -> Result<impl Stream<Item = crate::gql::types::TournamentClock>, async_graphql::Error> {
        let subscription = crate::gql::tournament_clock::TournamentClockSubscription;
        subscription
            .tournament_clock_updates(ctx, tournament_id)
            .await
    }

    /// Subscribe to player registration events for all tournaments
    async fn tournament_registrations(
        &self,
    ) -> impl Stream<Item = Result<PlayerRegistrationEvent, BroadcastStreamRecvError>> {
        let receiver = REGISTRATION_BROADCASTER.lock().unwrap().subscribe();
        BroadcastStream::new(receiver)
    }

    /// Subscribe to seating changes for a specific tournament
    async fn tournament_seating_changes(
        &self,
        tournament_id: async_graphql::ID,
    ) -> impl Stream<Item = Result<SeatingChangeEvent, BroadcastStreamRecvError>> {
        let receiver = SEATING_BROADCASTER.lock().unwrap().subscribe();
        let tournament_id_filter = tournament_id.to_string();

        tokio_stream::StreamExt::filter(BroadcastStream::new(receiver), move |event| {
            match event {
                Ok(seating_event) => seating_event.tournament_id.as_str() == tournament_id_filter,
                Err(_) => true, // Let errors through
            }
        })
    }

    /// Subscribe to seating changes for all tournaments in manager's club (managers only)
    async fn club_seating_changes(
        &self,
        ctx: &Context<'_>,
        club_id: async_graphql::ID,
    ) -> Result<impl Stream<Item = Result<SeatingChangeEvent, BroadcastStreamRecvError>>> {
        use crate::auth::permissions::require_role;
        use crate::gql::types::Role;

        // Require manager role
        let _manager = require_role(ctx, Role::Manager).await?;

        // TODO: Verify manager belongs to this club (would need club_user relationship)
        // For now, we trust the manager can only manage their own club

        let receiver = SEATING_BROADCASTER.lock().unwrap().subscribe();
        let club_id_filter = club_id.to_string();

        // Filter events by club_id
        Ok(tokio_stream::StreamExt::filter(
            BroadcastStream::new(receiver),
            move |event| {
                match event {
                    Ok(seating_event) => seating_event.club_id.as_str() == club_id_filter,
                    Err(_) => true, // Let errors through
                }
            },
        ))
    }
}

pub fn publish_registration_event(event: PlayerRegistrationEvent) {
    if let Ok(sender) = REGISTRATION_BROADCASTER.lock() {
        let _ = sender.send(event);
    }
}

pub fn publish_seating_event(event: SeatingChangeEvent) {
    if let Ok(sender) = SEATING_BROADCASTER.lock() {
        let _ = sender.send(event);
    }
}
