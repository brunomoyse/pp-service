pub mod resolvers;
pub mod types;

pub use resolvers::ActivityLogQuery;

use crate::gql::subscriptions::publish_activity_event;
use sqlx::PgPool;
use types::ActivityLogEntry;
use uuid::Uuid;

/// Persist an activity log entry and broadcast it via subscription.
/// Errors are logged but not propagated — callers should not fail due to logging.
pub async fn log_and_publish(
    pool: &PgPool,
    tournament_id: Uuid,
    event_category: &str,
    event_action: &str,
    actor_id: Option<Uuid>,
    subject_id: Option<Uuid>,
    metadata: serde_json::Value,
) {
    match infra::repos::activity_log::log_activity(
        pool,
        tournament_id,
        event_category,
        event_action,
        actor_id,
        subject_id,
        metadata,
    )
    .await
    {
        Ok(row) => {
            let entry = ActivityLogEntry::from(row);
            publish_activity_event(tournament_id, entry);
        }
        Err(e) => {
            tracing::error!(
                tournament_id = %tournament_id,
                event_category,
                event_action,
                "Failed to log activity: {e}"
            );
        }
    }
}
