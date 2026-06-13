//! Cross-instance real-time bus.
//!
//! Subscriptions are served from in-process Tokio broadcast channels
//! (`subscriptions::CHANNELS`). With more than one backend instance, an event
//! published on instance A must also reach subscribers connected to instance B.
//! Postgres LISTEN/NOTIFY is the bus:
//!
//! ```text
//! publish_*()  ->  queue()  ->  [notifier task]  ->  pg_notify('pp_realtime', json)
//!                     |                                        |
//!                     | (local, immediate)                    v
//!                     +--> dispatch_local() <--- [listener task on every instance]
//! ```
//!
//! Each instance has a unique `INSTANCE_ID`. The notifier dispatches the event
//! to local subscribers immediately (no DB round-trip, no loss if the listener
//! is mid-reconnect) AND publishes it tagged with the origin id. Every
//! instance's listener receives it but skips its own origin, so remote
//! subscribers get it exactly once.

use std::sync::{LazyLock, OnceLock};

use serde::{Deserialize, Serialize};
use sqlx::postgres::PgListener;
use sqlx::PgPool;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::gql::subscriptions::dispatch_local;
use crate::gql::types::{
    ActivityLogEntry, PlayerRegistrationEvent, SeatingChangeEvent, TournamentClock,
    UserNotification,
};

/// Postgres NOTIFY channel name.
const CHANNEL: &str = "pp_realtime";

/// NOTIFY payloads are capped at 8000 bytes by Postgres; stay under that.
const MAX_PAYLOAD_BYTES: usize = 7500;

/// Identifies this process so the listener can skip events it published itself.
static INSTANCE_ID: LazyLock<Uuid> = LazyLock::new(Uuid::new_v4);

/// Set once the notifier task is running; `publish_*` enqueue through it.
static SENDER: OnceLock<mpsc::UnboundedSender<RealtimeEvent>> = OnceLock::new();

/// A real-time event plus the routing the local dispatcher needs. Routing that
/// isn't already carried inside the event payload (clock/activity tournament id)
/// is captured explicitly.
#[derive(Clone, Serialize, Deserialize)]
pub enum RealtimeEvent {
    Registration(PlayerRegistrationEvent),
    Seating(SeatingChangeEvent),
    Clock {
        tournament_id: Uuid,
        clock: Box<TournamentClock>,
    },
    Activity {
        tournament_id: Uuid,
        entry: ActivityLogEntry,
    },
    UserNotification(UserNotification),
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    origin: Uuid,
    event: RealtimeEvent,
}

/// Hand an event to the bus. When the notifier is running the event goes through
/// Postgres (and is dispatched locally by the notifier); otherwise — e.g. in
/// tests with no bus — it is dispatched in-process immediately so local
/// subscribers still receive it.
pub(crate) fn queue(event: RealtimeEvent) {
    match SENDER.get() {
        Some(tx) => {
            let _ = tx.send(event);
        }
        None => dispatch_local(event),
    }
}

/// Drain queued events: dispatch to local subscribers and broadcast to other
/// instances via pg_notify. Spawned once at startup.
pub fn spawn_realtime_notifier(db: PgPool) -> JoinHandle<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<RealtimeEvent>();
    if SENDER.set(tx).is_err() {
        tracing::warn!("realtime notifier already initialized; ignoring duplicate spawn");
    }

    tokio::spawn(async move {
        tracing::info!("Realtime notifier started (instance {})", *INSTANCE_ID);
        while let Some(event) = rx.recv().await {
            // Local subscribers get it immediately, independent of the DB.
            dispatch_local(event.clone());

            let envelope = Envelope {
                origin: *INSTANCE_ID,
                event,
            };
            let payload = match serde_json::to_string(&envelope) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("realtime: failed to serialize event: {e}");
                    continue;
                }
            };
            if payload.len() > MAX_PAYLOAD_BYTES {
                // Local delivery already happened; only cross-instance fan-out is
                // skipped. Rare for these event shapes — log so it's visible.
                tracing::warn!(
                    "realtime: event too large for NOTIFY ({} > {} bytes); not broadcast to other instances",
                    payload.len(),
                    MAX_PAYLOAD_BYTES
                );
                continue;
            }
            if let Err(e) = sqlx::query("SELECT pg_notify($1, $2)")
                .bind(CHANNEL)
                .bind(&payload)
                .execute(&db)
                .await
            {
                tracing::error!("realtime: pg_notify failed: {e}");
            }
        }
        tracing::warn!("Realtime notifier channel closed; task exiting");
    })
}

/// Listen for events published by other instances and fan them into the local
/// broadcast channels. Reconnects on connection loss. Spawned once at startup.
pub fn spawn_realtime_listener(db: PgPool) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match run_listener(&db).await {
                Ok(()) => {
                    tracing::warn!("Realtime listener stream ended; reconnecting in 3s");
                }
                Err(e) => {
                    tracing::error!("Realtime listener error: {e}; reconnecting in 3s");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    })
}

async fn run_listener(db: &PgPool) -> Result<(), sqlx::Error> {
    let mut listener = PgListener::connect_with(db).await?;
    listener.listen(CHANNEL).await?;
    tracing::info!("Realtime listener subscribed to '{CHANNEL}'");

    loop {
        let notification = listener.recv().await?;
        let envelope: Envelope = match serde_json::from_str(notification.payload()) {
            Ok(env) => env,
            Err(e) => {
                tracing::error!("realtime: failed to decode notification: {e}");
                continue;
            }
        };
        // Our own events were already dispatched locally by the notifier.
        if envelope.origin == *INSTANCE_ID {
            continue;
        }
        dispatch_local(envelope.event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gql::types::{NotificationType, UserNotification};

    // The NOTIFY payload is a JSON Envelope; an event must survive
    // serialize -> deserialize so a remote instance reconstructs it faithfully.
    #[test]
    fn envelope_round_trips() {
        let event = RealtimeEvent::UserNotification(UserNotification {
            id: "notif-1".into(),
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            notification_type: NotificationType::PlayerEliminated,
            title: "Busted".into(),
            message: "You finished 5th".into(),
            tournament_id: Some("22222222-2222-2222-2222-222222222222".into()),
            created_at: chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        });
        let origin = Uuid::nil();
        let json = serde_json::to_string(&Envelope { origin, event }).unwrap();
        let back: Envelope = serde_json::from_str(&json).unwrap();

        assert_eq!(back.origin, origin);
        match back.event {
            RealtimeEvent::UserNotification(n) => {
                assert_eq!(n.id.as_str(), "notif-1");
                assert_eq!(n.title, "Busted");
                assert_eq!(n.notification_type, NotificationType::PlayerEliminated);
            }
            _ => panic!("variant changed across round-trip"),
        }
    }
}
