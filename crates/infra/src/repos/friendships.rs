use sqlx::{PgExecutor, Result as SqlxResult};
use uuid::Uuid;

use crate::models::{FlameRow, FriendRow, FriendshipRow};

const FRIENDSHIP_COLS: &str = "id, requester_id, addressee_id, status, created_at, updated_at";

/// The friendship between two users, in whichever direction it exists.
pub async fn get_between<'e>(
    executor: impl PgExecutor<'e>,
    a: Uuid,
    b: Uuid,
) -> SqlxResult<Option<FriendshipRow>> {
    sqlx::query_as::<_, FriendshipRow>(&format!(
        "SELECT {FRIENDSHIP_COLS} FROM friendship \
         WHERE (requester_id = $1 AND addressee_id = $2) \
            OR (requester_id = $2 AND addressee_id = $1)"
    ))
    .bind(a)
    .bind(b)
    .fetch_optional(executor)
    .await
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> SqlxResult<Option<FriendshipRow>> {
    sqlx::query_as::<_, FriendshipRow>(&format!(
        "SELECT {FRIENDSHIP_COLS} FROM friendship WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn create_request<'e>(
    executor: impl PgExecutor<'e>,
    requester_id: Uuid,
    addressee_id: Uuid,
) -> SqlxResult<FriendshipRow> {
    sqlx::query_as::<_, FriendshipRow>(&format!(
        "INSERT INTO friendship (requester_id, addressee_id) VALUES ($1, $2) \
         RETURNING {FRIENDSHIP_COLS}"
    ))
    .bind(requester_id)
    .bind(addressee_id)
    .fetch_one(executor)
    .await
}

/// Accept a pending request — only the addressee may accept.
pub async fn accept<'e>(
    executor: impl PgExecutor<'e>,
    friendship_id: Uuid,
    addressee_id: Uuid,
) -> SqlxResult<Option<FriendshipRow>> {
    sqlx::query_as::<_, FriendshipRow>(&format!(
        "UPDATE friendship SET status = 'accepted' \
         WHERE id = $1 AND addressee_id = $2 AND status = 'pending' \
         RETURNING {FRIENDSHIP_COLS}"
    ))
    .bind(friendship_id)
    .bind(addressee_id)
    .fetch_optional(executor)
    .await
}

/// Remove/decline a friendship — either party may do so.
pub async fn remove<'e>(
    executor: impl PgExecutor<'e>,
    friendship_id: Uuid,
    user_id: Uuid,
) -> SqlxResult<bool> {
    let res = sqlx::query(
        "DELETE FROM friendship \
         WHERE id = $1 AND (requester_id = $2 OR addressee_id = $2)",
    )
    .bind(friendship_id)
    .bind(user_id)
    .execute(executor)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Accepted friends, resolved to the other party.
pub async fn list_friends<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
) -> SqlxResult<Vec<FriendRow>> {
    sqlx::query_as::<_, FriendRow>(
        "SELECT f.id AS friendship_id, \
                other.id AS user_id, \
                COALESCE(other.username, other.first_name) AS name, \
                f.status AS status, \
                FALSE AS is_incoming \
         FROM friendship f \
         JOIN users other ON other.id = \
              CASE WHEN f.requester_id = $1 THEN f.addressee_id ELSE f.requester_id END \
         WHERE (f.requester_id = $1 OR f.addressee_id = $1) AND f.status = 'accepted' \
         ORDER BY name ASC",
    )
    .bind(user_id)
    .fetch_all(executor)
    .await
}

/// Pending requests the current user has received.
pub async fn list_incoming<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
) -> SqlxResult<Vec<FriendRow>> {
    sqlx::query_as::<_, FriendRow>(
        "SELECT f.id AS friendship_id, \
                u.id AS user_id, \
                COALESCE(u.username, u.first_name) AS name, \
                f.status AS status, \
                TRUE AS is_incoming \
         FROM friendship f JOIN users u ON u.id = f.requester_id \
         WHERE f.addressee_id = $1 AND f.status = 'pending' \
         ORDER BY f.created_at DESC",
    )
    .bind(user_id)
    .fetch_all(executor)
    .await
}

/// Pending requests the current user has sent (awaiting the other party).
pub async fn list_outgoing<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
) -> SqlxResult<Vec<FriendRow>> {
    sqlx::query_as::<_, FriendRow>(
        "SELECT f.id AS friendship_id, \
                u.id AS user_id, \
                COALESCE(u.username, u.first_name) AS name, \
                f.status AS status, \
                FALSE AS is_incoming \
         FROM friendship f JOIN users u ON u.id = f.addressee_id \
         WHERE f.requester_id = $1 AND f.status = 'pending' \
         ORDER BY f.created_at DESC",
    )
    .bind(user_id)
    .fetch_all(executor)
    .await
}

/// The mutual flame between two players: how many distinct nights both checked
/// in, and the most recent such night.
pub async fn flame_between<'e>(
    executor: impl PgExecutor<'e>,
    a: Uuid,
    b: Uuid,
) -> SqlxResult<FlameRow> {
    sqlx::query_as::<_, FlameRow>(
        "SELECT COUNT(*) AS shared_nights, MAX(day) AS last_shared FROM ( \
             SELECT checked_in_at::date AS day FROM check_in WHERE app_user_id = $1 \
             INTERSECT \
             SELECT checked_in_at::date AS day FROM check_in WHERE app_user_id = $2 \
         ) shared",
    )
    .bind(a)
    .bind(b)
    .fetch_one(executor)
    .await
}
