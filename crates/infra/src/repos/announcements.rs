use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::AnnouncementRow;
use crate::pagination::LimitOffset;

const COLUMNS: &str =
    "id, scope, club_id, tournament_id, title, body, created_by, created_at, updated_at";

/// Persist a new announcement. `club_id`/`tournament_id` must match the scope
/// (enforced by the table CHECK constraint).
pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    scope: &str,
    club_id: Option<Uuid>,
    tournament_id: Option<Uuid>,
    title: &str,
    body: &str,
    created_by: Uuid,
) -> SqlxResult<AnnouncementRow> {
    sqlx::query_as::<_, AnnouncementRow>(&format!(
        "INSERT INTO announcements (scope, club_id, tournament_id, title, body, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING {COLUMNS}"
    ))
    .bind(scope)
    .bind(club_id)
    .bind(tournament_id)
    .bind(title)
    .bind(body)
    .bind(created_by)
    .fetch_one(executor)
    .await
}

/// The predicate matching every announcement an app user should see in their
/// feed: any platform announcement, club announcements for clubs they are a
/// claimed active roster member of, and tournament announcements for tournaments
/// they have a live (non-cancelled/no_show) registration in. `$1` is the user id.
const FEED_PREDICATE: &str = "(\
    a.scope = 'platform' \
    OR (a.scope = 'club' AND a.club_id IN ( \
        SELECT cp.club_id FROM club_player cp \
        WHERE cp.app_user_id = $1 AND cp.is_active = true)) \
    OR (a.scope = 'tournament' AND a.tournament_id IN ( \
        SELECT tr.tournament_id FROM tournament_registrations tr \
        LEFT JOIN club_player cp ON cp.id = tr.club_player_id \
        WHERE tr.status NOT IN ('cancelled', 'no_show') \
          AND (tr.user_id = $1 OR cp.app_user_id = $1))) \
)";

/// One page of the announcements feed visible to `user_id`, newest first.
pub async fn list_for_user<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
    page: LimitOffset,
) -> SqlxResult<Vec<AnnouncementRow>> {
    sqlx::query_as::<_, AnnouncementRow>(&format!(
        "SELECT {COLUMNS} FROM announcements a WHERE {FEED_PREDICATE} \
         ORDER BY a.created_at DESC LIMIT $2 OFFSET $3"
    ))
    .bind(user_id)
    .bind(page.limit)
    .bind(page.offset)
    .fetch_all(executor)
    .await
}

/// Total announcements visible to `user_id` (for pagination).
pub async fn count_for_user<'e>(executor: impl PgExecutor<'e>, user_id: Uuid) -> SqlxResult<i64> {
    sqlx::query_scalar::<_, i64>(&format!(
        "SELECT COUNT(*) FROM announcements a WHERE {FEED_PREDICATE}"
    ))
    .bind(user_id)
    .fetch_one(executor)
    .await
}

/// One page of a club's announcements (its tournament + club scoped rows),
/// newest first — the manager management view.
pub async fn list_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    page: LimitOffset,
) -> SqlxResult<Vec<AnnouncementRow>> {
    sqlx::query_as::<_, AnnouncementRow>(&format!(
        "SELECT {COLUMNS} FROM announcements WHERE club_id = $1 \
         ORDER BY created_at DESC LIMIT $2 OFFSET $3"
    ))
    .bind(club_id)
    .bind(page.limit)
    .bind(page.offset)
    .fetch_all(executor)
    .await
}

/// Total announcements for a club (for pagination).
pub async fn count_by_club<'e>(executor: impl PgExecutor<'e>, club_id: Uuid) -> SqlxResult<i64> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM announcements WHERE club_id = $1")
        .bind(club_id)
        .fetch_one(executor)
        .await
}

/// Resolve the push audience for an announcement: every `(user_id, token, locale)`
/// device of every eligible user, with the `announcements` notification
/// preference honoured in SQL (`COALESCE(..., TRUE)` so a user who never set
/// preferences still receives them). The scope picks the audience set.
pub async fn audience_device_tokens<'e>(
    executor: impl PgExecutor<'e>,
    announcement: &AnnouncementRow,
) -> SqlxResult<Vec<(Uuid, String, Option<String>)>> {
    match announcement.scope.as_str() {
        "platform" => {
            sqlx::query_as::<_, (Uuid, String, Option<String>)>(
                "SELECT DISTINCT u.id, dt.token, dt.locale \
                 FROM users u \
                 JOIN device_tokens dt ON dt.user_id = u.id \
                 LEFT JOIN notification_preferences np ON np.user_id = u.id \
                 WHERE u.role = 'player' AND u.is_active = true \
                   AND COALESCE(np.announcements, TRUE) = TRUE",
            )
            .fetch_all(executor)
            .await
        }
        "club" => {
            sqlx::query_as::<_, (Uuid, String, Option<String>)>(
                "SELECT DISTINCT u.id, dt.token, dt.locale \
                 FROM club_player cp \
                 JOIN users u ON u.id = cp.app_user_id \
                 JOIN device_tokens dt ON dt.user_id = u.id \
                 LEFT JOIN notification_preferences np ON np.user_id = u.id \
                 WHERE cp.club_id = $1 AND cp.is_active = true AND cp.app_user_id IS NOT NULL \
                   AND u.is_active = true \
                   AND COALESCE(np.announcements, TRUE) = TRUE",
            )
            .bind(announcement.club_id)
            .fetch_all(executor)
            .await
        }
        _ => {
            // tournament
            sqlx::query_as::<_, (Uuid, String, Option<String>)>(
                "SELECT DISTINCT u.id, dt.token, dt.locale \
                 FROM tournament_registrations tr \
                 LEFT JOIN club_player cp ON cp.id = tr.club_player_id \
                 JOIN users u ON u.id = COALESCE(tr.user_id, cp.app_user_id) \
                 JOIN device_tokens dt ON dt.user_id = u.id \
                 LEFT JOIN notification_preferences np ON np.user_id = u.id \
                 WHERE tr.tournament_id = $1 \
                   AND tr.status NOT IN ('cancelled', 'no_show') \
                   AND u.is_active = true \
                   AND COALESCE(np.announcements, TRUE) = TRUE",
            )
            .bind(announcement.tournament_id)
            .fetch_all(executor)
            .await
        }
    }
}
