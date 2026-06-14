use sqlx::PgPool;
use uuid::Uuid;

use crate::gql::error::GqlError;
use crate::services::push_service;
use infra::models::AnnouncementRow;
use infra::repos::announcements;

use super::types::AnnouncementScope;

/// Persist an announcement and fan its push out to the resolved audience.
///
/// Validation only (auth + club derivation happen in the resolver). The push
/// is best-effort and fire-and-forget — a delivery failure never fails the
/// mutation, and the persisted row remains readable in the in-app feed.
pub async fn create_announcement(
    db: &PgPool,
    scope: AnnouncementScope,
    club_id: Option<Uuid>,
    tournament_id: Option<Uuid>,
    title: &str,
    body: &str,
    created_by: Uuid,
) -> Result<AnnouncementRow, GqlError> {
    let title = title.trim();
    let body = body.trim();
    if title.is_empty() {
        return Err(GqlError::new("Title cannot be empty"));
    }
    if body.is_empty() {
        return Err(GqlError::new("Body cannot be empty"));
    }

    let row = announcements::create(
        db,
        scope.as_str(),
        club_id,
        tournament_id,
        title,
        body,
        created_by,
    )
    .await?;

    // Fan out the push without blocking the mutation response.
    let db = db.clone();
    let row_for_push = row.clone();
    tokio::spawn(async move {
        match announcements::audience_device_tokens(&db, &row_for_push).await {
            Ok(audience) => {
                push_service::send_announcement(
                    &db,
                    audience,
                    row_for_push.id,
                    row_for_push.tournament_id,
                    &row_for_push.title,
                    &row_for_push.body,
                )
                .await;
            }
            Err(e) => {
                tracing::warn!(
                    announcement_id = %row_for_push.id,
                    error = %e,
                    "announcement: failed to resolve push audience"
                );
            }
        }
    });

    Ok(row)
}
