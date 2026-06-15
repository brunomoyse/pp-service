//! Recurring-tournament expansion.
//!
//! A manager who runs the same event on a fixed cadence can create the whole
//! run in one go: pick a frequency and an end date, and we expand that into a
//! list of occurrence start times. The occurrences are then created as ordinary
//! independent tournaments (no recurrence entity, no scheduler) — see
//! `create_tournament`.
//!
//! This module is pure (no DB) so the date arithmetic is unit-testable in
//! isolation; `occurrence_starts` is the only thing the resolver calls.

use async_graphql::Enum;
use chrono::{DateTime, Months, Utc};

/// How often a recurring tournament repeats.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum RecurrenceFrequency {
    /// Every 7 days, same time of day.
    Weekly,
    /// Same day-of-month next month (clamped for short months, e.g. Jan 31 → Feb 28).
    Monthly,
}

/// Safety ceiling on how many occurrences a single request may create, so a
/// far-off end date can't spawn thousands of tournaments. Two years of weekly
/// events comfortably fits.
pub const MAX_OCCURRENCES: usize = 104;

/// Expand a recurrence spec into the list of occurrence start times.
///
/// The original `start` is occurrence 1; subsequent occurrences step by `freq`
/// and are included while `<= end`. Returns an error if `end` precedes `start`
/// or if the run would exceed `cap`.
pub fn occurrence_starts(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    freq: RecurrenceFrequency,
    cap: usize,
) -> Result<Vec<DateTime<Utc>>, String> {
    if end < start {
        return Err("Recurrence end date must be on or after the start time".to_string());
    }

    let mut out = Vec::new();
    let mut current = start;
    while current <= end {
        out.push(current);
        if out.len() > cap {
            return Err(format!(
                "Recurrence would create more than {cap} tournaments; choose a closer end date"
            ));
        }
        current = match freq {
            RecurrenceFrequency::Weekly => current + chrono::Duration::weeks(1),
            // checked_add_months preserves time-of-day and clamps overflowing
            // days (e.g. Jan 31 + 1 month → Feb 28). Failure is unreachable for
            // sane dates, but if it ever happens we stop expanding.
            RecurrenceFrequency::Monthly => match current.checked_add_months(Months::new(1)) {
                Some(next) => next,
                None => break,
            },
        };
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn dt(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    #[test]
    fn weekly_includes_endpoints() {
        // Start + 4 weeks out (inclusive) → 5 occurrences, 7 days apart.
        let start = dt(2026, 1, 1, 19);
        let end = dt(2026, 1, 29, 19);
        let starts = occurrence_starts(start, end, RecurrenceFrequency::Weekly, MAX_OCCURRENCES)
            .expect("valid");
        assert_eq!(starts.len(), 5);
        assert_eq!(starts[0], start);
        assert_eq!(starts[4], end);
        for w in starts.windows(2) {
            assert_eq!(w[1] - w[0], chrono::Duration::days(7));
        }
    }

    #[test]
    fn weekly_excludes_just_past_end() {
        // End one day before the 5th occurrence → only 4.
        let start = dt(2026, 1, 1, 19);
        let end = dt(2026, 1, 28, 19);
        let starts =
            occurrence_starts(start, end, RecurrenceFrequency::Weekly, MAX_OCCURRENCES).unwrap();
        assert_eq!(starts.len(), 4);
    }

    #[test]
    fn monthly_preserves_day_of_month() {
        let start = dt(2026, 1, 15, 20);
        let end = dt(2026, 4, 15, 20);
        let starts =
            occurrence_starts(start, end, RecurrenceFrequency::Monthly, MAX_OCCURRENCES).unwrap();
        assert_eq!(starts.len(), 4);
        assert_eq!(
            starts,
            vec![start, dt(2026, 2, 15, 20), dt(2026, 3, 15, 20), end]
        );
    }

    #[test]
    fn monthly_clamps_short_months() {
        // Jan 31 → Feb 28 (2026 is not a leap year) → Mar 28 → ...
        let start = dt(2026, 1, 31, 20);
        let end = dt(2026, 3, 31, 20);
        let starts =
            occurrence_starts(start, end, RecurrenceFrequency::Monthly, MAX_OCCURRENCES).unwrap();
        assert_eq!(starts[0], dt(2026, 1, 31, 20));
        assert_eq!(starts[1], dt(2026, 2, 28, 20));
    }

    #[test]
    fn end_equal_start_is_single_occurrence() {
        let start = dt(2026, 1, 1, 19);
        let starts =
            occurrence_starts(start, start, RecurrenceFrequency::Weekly, MAX_OCCURRENCES).unwrap();
        assert_eq!(starts.len(), 1);
    }

    #[test]
    fn end_before_start_errors() {
        let start = dt(2026, 1, 8, 19);
        let end = dt(2026, 1, 1, 19);
        assert!(
            occurrence_starts(start, end, RecurrenceFrequency::Weekly, MAX_OCCURRENCES).is_err()
        );
    }

    #[test]
    fn cap_exceeded_errors() {
        let start = dt(2026, 1, 1, 19);
        let end = dt(2030, 1, 1, 19); // ~209 weeks
        assert!(occurrence_starts(start, end, RecurrenceFrequency::Weekly, 104).is_err());
    }
}
