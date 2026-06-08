//! Delivers push notifications to a user's devices via the Expo Push API.
//!
//! Tokens are minted client-side (`getExpoPushTokenAsync`) and stored in
//! `device_tokens`. Sending is best-effort and fire-and-forget: failures are
//! logged, never surfaced to the caller. Tokens Expo reports as
//! `DeviceNotRegistered` are pruned so we stop targeting dead installs.

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use infra::repos::device_tokens;

const EXPO_PUSH_URL: &str = "https://exp.host/--/api/v2/push/send";

/// Send a push to every device registered for `user_id`.
///
/// `data` is delivered alongside the notification so the app can route the tap
/// (e.g. `{ "type": "ACHIEVEMENT_UNLOCKED", "code": "first_win" }`).
pub async fn send_to_user(
    db: &PgPool,
    user_id: Uuid,
    title: &str,
    body: &str,
    data: serde_json::Value,
) {
    let tokens = match device_tokens::list_for_user(db, user_id).await {
        Ok(t) if !t.is_empty() => t,
        Ok(_) => return,
        Err(e) => {
            tracing::warn!(%user_id, error = %e, "push: failed to load device tokens");
            return;
        }
    };

    let messages: Vec<serde_json::Value> = tokens
        .iter()
        .map(|token| {
            json!({
                "to": token,
                "title": title,
                "body": body,
                "data": data,
                "sound": "default",
                "channelId": "default",
                "priority": "high",
            })
        })
        .collect();

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

    // Receipts come back in `data` in the same order as the tickets we sent;
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
