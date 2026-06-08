//! Delivers push notifications to a user's devices via the Expo Push API.
//!
//! Tokens are minted client-side (`getExpoPushTokenAsync`) and stored in
//! `device_tokens` along with the device locale. Sending is best-effort and
//! fire-and-forget: failures are logged, never surfaced to the caller. Tokens
//! Expo reports as `DeviceNotRegistered` are pruned so we stop targeting dead
//! installs.

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use infra::repos::device_tokens;

const EXPO_PUSH_URL: &str = "https://exp.host/--/api/v2/push/send";

/// Localized generic push copy. The achievement's localized *name* lives in the
/// app's i18n bundle (not on the server), so the push body is intentionally
/// generic; tapping it deep-links to the achievements screen.
fn achievement_copy(locale: Option<&str>) -> (&'static str, &'static str) {
    match locale.unwrap_or("en") {
        "fr" => (
            "Succès débloqué",
            "Vous avez débloqué un nouveau succès — touchez pour voir.",
        ),
        "nl" => (
            "Prestatie ontgrendeld",
            "Je hebt een nieuwe prestatie ontgrendeld — tik om te bekijken.",
        ),
        _ => (
            "Achievement Unlocked",
            "You've earned a new achievement — tap to view.",
        ),
    }
}

/// Push an achievement-unlock alert to every device registered for `user_id`,
/// localized per device. `data.code` lets the app deep-link to the achievement.
pub async fn send_achievement_unlock(db: &PgPool, user_id: Uuid, code: &str, name_key: &str) {
    let devices = match device_tokens::list_for_user(db, user_id).await {
        Ok(d) if !d.is_empty() => d,
        Ok(_) => return,
        Err(e) => {
            tracing::warn!(%user_id, error = %e, "push: failed to load device tokens");
            return;
        }
    };

    let data = json!({
        "type": "ACHIEVEMENT_UNLOCKED",
        "code": code,
        "name_key": name_key,
    });

    let tokens: Vec<String> = devices.iter().map(|d| d.token.clone()).collect();
    let messages: Vec<serde_json::Value> = devices
        .iter()
        .map(|d| {
            let (title, body) = achievement_copy(d.locale.as_deref());
            json!({
                "to": d.token,
                "title": title,
                "body": body,
                "data": data,
                "sound": "default",
                "channelId": "default",
                "priority": "high",
            })
        })
        .collect();

    deliver(db, user_id, messages, tokens).await;
}

/// POST a batch of Expo messages and prune any tokens reported as dead.
/// `tokens` is parallel to `messages` so error receipts map back to a token.
async fn deliver(
    db: &PgPool,
    user_id: Uuid,
    messages: Vec<serde_json::Value>,
    tokens: Vec<String>,
) {
    let client = reqwest::Client::new();
    let mut req = client
        .post(EXPO_PUSH_URL)
        .header("accept", "application/json")
        .header("content-type", "application/json");
    // Optional: an Expo access token enforces "Enhanced Security" on the project.
    if let Ok(access) = std::env::var("EXPO_ACCESS_TOKEN") {
        if !access.is_empty() {
            req = req.bearer_auth(access);
        }
    }

    let resp = match req.json(&messages).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(%user_id, error = %e, "push: Expo request failed");
            return;
        }
    };

    let payload: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(%user_id, error = %e, "push: invalid Expo response");
            return;
        }
    };

    // Receipts come back in `data` in the same order as the messages we sent;
    // an error with details.error == "DeviceNotRegistered" means the token is
    // dead and should be removed.
    let mut dead: Vec<String> = Vec::new();
    if let Some(tickets) = payload.get("data").and_then(|d| d.as_array()) {
        for (ticket, token) in tickets.iter().zip(tokens.iter()) {
            if ticket.get("status").and_then(|s| s.as_str()) == Some("error") {
                let reason = ticket
                    .get("details")
                    .and_then(|d| d.get("error"))
                    .and_then(|e| e.as_str());
                if reason == Some("DeviceNotRegistered") {
                    dead.push(token.clone());
                }
                tracing::warn!(%user_id, ?reason, "push: Expo rejected a message");
            }
        }
    }

    if !dead.is_empty() {
        if let Err(e) = device_tokens::delete_tokens(db, &dead).await {
            tracing::warn!(error = %e, "push: failed to prune dead tokens");
        }
    }
}
