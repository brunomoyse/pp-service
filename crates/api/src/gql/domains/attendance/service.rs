use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::gql::common::helpers::get_club_id_for_tournament;
use crate::gql::error::GqlError;
use infra::db::Db;
use infra::models::AttendanceStreakRow;
use infra::repos::attendance;

/// Gap (since the last check-in) within which the streak simply advances.
const ON_TIME_DAYS: i64 = 8;
/// Gap within which a missed week can be forgiven by spending a freeze.
const FREEZE_WINDOW_DAYS: i64 = 15;
/// Beyond this, returning counts as a "comeback".
const COMEBACK_DAYS: i64 = 21;
/// Earn a freeze back every N on-time check-ins.
const FREEZE_EARN_INTERVAL: i32 = 4;
const MAX_FREEZES: i32 = 2;

pub struct CheckInOutcome {
    pub streak: AttendanceStreakRow,
    pub already_checked_in: bool,
    pub freeze_used: bool,
    pub is_comeback: bool,
    pub is_new_longest: bool,
}

fn default_streak(user_id: Uuid) -> AttendanceStreakRow {
    let now = Utc::now();
    AttendanceStreakRow {
        app_user_id: user_id,
        current_streak: 0,
        longest_streak: 0,
        last_check_in_at: None,
        freezes_available: MAX_FREEZES,
        created_at: now,
        updated_at: now,
    }
}

/// Record a check-in for `user_id` at `tournament_id` and advance their
/// attendance streak. Idempotent: a second check-in for the same tournament
/// leaves the streak untouched and reports `already_checked_in`.
pub async fn record_check_in(
    db: &Db,
    user_id: Uuid,
    tournament_id: Uuid,
) -> Result<CheckInOutcome, GqlError> {
    let club_id = get_club_id_for_tournament(db, tournament_id)
        .await
        .map_err(|e| GqlError::new(e.message))?;

    let newly = attendance::record_check_in(db, user_id, tournament_id, club_id).await?;
    let existing = attendance::get_streak(db, user_id).await?;

    // Already checked in for this tournament — no streak movement.
    if newly.is_none() {
        let streak = existing.unwrap_or_else(|| default_streak(user_id));
        return Ok(CheckInOutcome {
            streak,
            already_checked_in: true,
            freeze_used: false,
            is_comeback: false,
            is_new_longest: false,
        });
    }

    let now = Utc::now();
    let (cur, longest, last, freezes) = match &existing {
        None => (0, 0, None, MAX_FREEZES),
        Some(s) => (
            s.current_streak,
            s.longest_streak,
            s.last_check_in_at,
            s.freezes_available,
        ),
    };

    let (mut new_current, freeze_used, is_comeback) = match last {
        None => (1, false, false),
        Some(last_at) => {
            let gap = now - last_at;
            if gap <= Duration::days(ON_TIME_DAYS) {
                (cur + 1, false, false)
            } else if gap <= Duration::days(FREEZE_WINDOW_DAYS) && freezes > 0 {
                // Forgive the missed week by burning a freeze.
                (cur + 1, true, false)
            } else {
                (1, false, gap > Duration::days(COMEBACK_DAYS))
            }
        }
    };
    if new_current < 1 {
        new_current = 1;
    }

    let mut new_freezes = if freeze_used { freezes - 1 } else { freezes };
    // Regenerate a freeze every few on-time check-ins (never on a freeze burn).
    if !freeze_used && new_current % FREEZE_EARN_INTERVAL == 0 {
        new_freezes += 1;
    }
    new_freezes = new_freezes.clamp(0, MAX_FREEZES);

    let new_longest = longest.max(new_current);
    let is_new_longest = new_current > longest && new_current >= 2;

    let streak =
        attendance::upsert_streak(db, user_id, new_current, new_longest, now, new_freezes).await?;

    Ok(CheckInOutcome {
        streak,
        already_checked_in: false,
        freeze_used,
        is_comeback,
        is_new_longest,
    })
}
