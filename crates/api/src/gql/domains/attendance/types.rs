use async_graphql::SimpleObject;
use chrono::{DateTime, Utc};

#[derive(SimpleObject, Clone, Debug)]
pub struct AttendanceStreak {
    pub current_streak: i32,
    pub longest_streak: i32,
    pub freezes_available: i32,
    pub last_check_in_at: Option<DateTime<Utc>>,
}

impl From<infra::models::AttendanceStreakRow> for AttendanceStreak {
    fn from(r: infra::models::AttendanceStreakRow) -> Self {
        Self {
            current_streak: r.current_streak,
            longest_streak: r.longest_streak,
            freezes_available: r.freezes_available,
            last_check_in_at: r.last_check_in_at,
        }
    }
}

/// Outcome of a check-in — the dopamine payload for the client.
#[derive(SimpleObject, Clone, Debug)]
pub struct CheckInResult {
    pub streak: AttendanceStreak,
    /// True if the player had already checked in (no streak change).
    pub already_checked_in: bool,
    /// A freeze was consumed to forgive a missed week.
    pub freeze_used: bool,
    /// The player was away 3+ weeks and is just back.
    pub is_comeback: bool,
    /// This check-in set a new personal-best streak.
    pub is_new_longest: bool,
}
