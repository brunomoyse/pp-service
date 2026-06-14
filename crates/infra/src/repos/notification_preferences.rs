use sqlx::{PgPool, Row};
use uuid::Uuid;

/// A user's notification preferences. A missing row means "never changed",
/// which is equivalent to all defaults (everything on).
#[derive(Clone, Copy, Debug)]
pub struct NotificationPreferences {
    pub tournament_reminders: bool,
    pub registration_updates: bool,
    pub seating_updates: bool,
    pub achievements: bool,
    pub announcements: bool,
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            tournament_reminders: true,
            registration_updates: true,
            seating_updates: true,
            achievements: true,
            announcements: true,
        }
    }
}

/// Preferences for a user, falling back to defaults when no row exists.
pub async fn get_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<NotificationPreferences, sqlx::Error> {
    let row = sqlx::query(
        "SELECT tournament_reminders, registration_updates, seating_updates, achievements, \
                announcements \
         FROM notification_preferences WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row
        .map(|r| NotificationPreferences {
            tournament_reminders: r.get("tournament_reminders"),
            registration_updates: r.get("registration_updates"),
            seating_updates: r.get("seating_updates"),
            achievements: r.get("achievements"),
            announcements: r.get("announcements"),
        })
        .unwrap_or_default())
}

/// Store the full set of preferences for a user (insert or update).
pub async fn upsert(
    pool: &PgPool,
    user_id: Uuid,
    prefs: NotificationPreferences,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO notification_preferences \
             (user_id, tournament_reminders, registration_updates, seating_updates, achievements, \
              announcements) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (user_id) DO UPDATE SET \
             tournament_reminders = EXCLUDED.tournament_reminders, \
             registration_updates = EXCLUDED.registration_updates, \
             seating_updates = EXCLUDED.seating_updates, \
             achievements = EXCLUDED.achievements, \
             announcements = EXCLUDED.announcements",
    )
    .bind(user_id)
    .bind(prefs.tournament_reminders)
    .bind(prefs.registration_updates)
    .bind(prefs.seating_updates)
    .bind(prefs.achievements)
    .bind(prefs.announcements)
    .execute(pool)
    .await?;

    Ok(())
}
