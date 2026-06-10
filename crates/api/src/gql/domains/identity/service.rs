use sqlx::PgPool;
use uuid::Uuid;

use crate::gql::error::GqlError;
use infra::models::ClubPlayerRow;
use infra::repos::club_players;

/// Create a roster entry for a non-app-user. Validates the display name.
pub async fn create_roster_entry(
    db: &PgPool,
    club_id: Uuid,
    display_name: &str,
) -> Result<ClubPlayerRow, GqlError> {
    let name = display_name.trim();
    if name.is_empty() {
        return Err(GqlError::new("Display name cannot be empty"));
    }
    Ok(club_players::create(db, club_id, name, None).await?)
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
    club_player_id: Uuid,
    app_user_id: Uuid,
) -> Result<ClubPlayerRow, GqlError> {
    let target = club_players::get_by_id(db, club_player_id)
        .await?
        .ok_or_else(|| GqlError::new("Roster entry not found"))?;

    if target.app_user_id == Some(app_user_id) {
        return Ok(target); // already claimed by this user — idempotent
    }
    if target.app_user_id.is_some() {
        return Err(GqlError::new("This roster entry has already been claimed"));
    }

    if let Some(existing) =
        club_players::find_by_club_and_app_user(db, target.club_id, app_user_id).await?
    {
        return Err(GqlError::new(format!(
            "You already have a profile in this club ({})",
            existing.display_name
        )));
    }

    club_players::claim(db, club_player_id, app_user_id)
        .await?
        .ok_or_else(|| GqlError::new("Roster entry could not be claimed"))
}

/// Rename a roster entry within its club. Validates the new display name.
pub async fn rename_roster_entry(
    db: &PgPool,
    id: Uuid,
    club_id: Uuid,
    display_name: &str,
) -> Result<ClubPlayerRow, GqlError> {
    let name = display_name.trim();
    if name.is_empty() {
        return Err(GqlError::new("Display name cannot be empty"));
    }
    club_players::update_display_name(db, id, club_id, name)
        .await?
        .ok_or_else(|| GqlError::new("Roster entry not found"))
}

/// Archive (soft-delete) or restore a roster entry within its club.
pub async fn set_roster_entry_active(
    db: &PgPool,
    id: Uuid,
    club_id: Uuid,
    is_active: bool,
) -> Result<ClubPlayerRow, GqlError> {
    club_players::set_active(db, id, club_id, is_active)
        .await?
        .ok_or_else(|| GqlError::new("Roster entry not found"))
}

/// A name that was not inserted during a bulk import, with the reason.
pub struct SkippedName {
    pub display_name: String,
    pub reason: String,
}

/// Bulk-create roster entries for a club from a list of display names.
///
/// De-duplicates empty names, repeats within the batch, and names that already
/// exist in the active roster (case-insensitive) — reporting each as skipped
/// rather than creating duplicates. Survivors are inserted in one transaction.
pub async fn create_roster_entries_bulk(
    db: &PgPool,
    club_id: Uuid,
    display_names: &[String],
) -> Result<(Vec<ClubPlayerRow>, Vec<SkippedName>), GqlError> {
    // Existing active roster names, lowercased, for case-insensitive dedupe.
    let existing = club_players::list_by_club(db, club_id).await?;
    let mut seen: std::collections::HashSet<String> = existing
        .iter()
        .map(|r| r.display_name.trim().to_lowercase())
        .collect();

    let mut created = Vec::new();
    let mut skipped = Vec::new();

    let mut tx = db.begin().await.map_err(|e| GqlError::new(e.to_string()))?;
    for raw in display_names {
        let name = raw.trim();
        if name.is_empty() {
            skipped.push(SkippedName {
                display_name: raw.clone(),
                reason: "Empty name".to_string(),
            });
            continue;
        }
        let key = name.to_lowercase();
        if !seen.insert(key) {
            skipped.push(SkippedName {
                display_name: name.to_string(),
                reason: "Duplicate (already in roster or repeated in file)".to_string(),
            });
            continue;
        }
        let row = club_players::create(&mut *tx, club_id, name, None)
            .await
            .map_err(|e| GqlError::new(e.to_string()))?;
        created.push(row);
    }
    tx.commit()
        .await
        .map_err(|e| GqlError::new(e.to_string()))?;

    Ok((created, skipped))
}
