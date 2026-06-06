use sqlx::PgPool;
use uuid::Uuid;

use crate::gql::error::GqlError;
use infra::models::RegisteredPlayerRow;
use infra::repos::registered_players;

/// Create a roster entry for a non-app-user. Validates the display name.
pub async fn create_roster_entry(
    db: &PgPool,
    club_id: Uuid,
    display_name: &str,
) -> Result<RegisteredPlayerRow, GqlError> {
    let name = display_name.trim();
    if name.is_empty() {
        return Err(GqlError::new("Display name cannot be empty"));
    }
    Ok(registered_players::create(db, club_id, name, None).await?)
}

/// Claim an unclaimed roster entry for an app user.
///
/// Merge semantics (spec §7.1): claiming only links the app user to the existing
/// roster entry; nothing else is disclosed. Idempotent if already claimed by the
/// same user. Refuses if the entry is claimed by someone else, or if the user
/// already has a (separate) claimed entry in that club — true row-merging of two
/// roster entries is a known follow-up (§7.2), not part of this foundation.
pub async fn claim_roster_entry(
    db: &PgPool,
    registered_player_id: Uuid,
    app_user_id: Uuid,
) -> Result<RegisteredPlayerRow, GqlError> {
    let target = registered_players::get_by_id(db, registered_player_id)
        .await?
        .ok_or_else(|| GqlError::new("Roster entry not found"))?;

    if target.app_user_id == Some(app_user_id) {
        return Ok(target); // already claimed by this user — idempotent
    }
    if target.app_user_id.is_some() {
        return Err(GqlError::new("This roster entry has already been claimed"));
    }

    if let Some(existing) =
        registered_players::find_by_club_and_app_user(db, target.club_id, app_user_id).await?
    {
        return Err(GqlError::new(format!(
            "You already have a profile in this club ({})",
            existing.display_name
        )));
    }

    registered_players::claim(db, registered_player_id, app_user_id)
        .await?
        .ok_or_else(|| GqlError::new("Roster entry could not be claimed"))
}
