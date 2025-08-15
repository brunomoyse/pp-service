use async_graphql::Subscription;
use futures_util::Stream;
use std::time::Duration;
use tokio::time::interval;
use tokio_stream::wrappers::{IntervalStream, BroadcastStream, errors::BroadcastStreamRecvError};
use tokio_stream::StreamExt;
use tokio::sync::broadcast;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

use crate::gql::types::PlayerRegistrationEvent;

static REGISTRATION_BROADCASTER: Lazy<Arc<Mutex<broadcast::Sender<PlayerRegistrationEvent>>>> =
    Lazy::new(|| {
        let (tx, _) = broadcast::channel(1000);
        Arc::new(Mutex::new(tx))
    });

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Simple ticking subscription (1..∞), useful as a template for live clock/announcements.
    async fn tick(&self) -> impl Stream<Item = i32> {
        let mut i = 0;
        IntervalStream::new(interval(Duration::from_secs(1)))
            .map(move |_| {
                i += 1;
                i
            })
    }

    /// Subscribe to player registration events for all tournaments
    async fn tournament_registrations(&self) -> impl Stream<Item = Result<PlayerRegistrationEvent, BroadcastStreamRecvError>> {
        let receiver = REGISTRATION_BROADCASTER.lock().unwrap().subscribe();
        BroadcastStream::new(receiver)
    }
}

pub fn publish_registration_event(event: PlayerRegistrationEvent) {
    if let Ok(sender) = REGISTRATION_BROADCASTER.lock() {
        let _ = sender.send(event);
    }
}