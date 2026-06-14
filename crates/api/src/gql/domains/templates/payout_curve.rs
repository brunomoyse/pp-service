//! Auto payout structures.
//!
//! Instead of hand-entering each paid position, a manager can ask for "pay the
//! top N%". We translate that into a pro-style **decaying** prize curve: the
//! number of paid places is `round(entrants * percent / 100)` (at least one),
//! and the prize pool is split with a geometric decay so the winner takes the
//! biggest share and each lower place a little less, down to a min-cash tail.
//!
//! The output percentages always sum to exactly 100.00 (rounding drift is
//! absorbed by first place), so they drop straight into a payout template /
//! `tournament_payouts` structure with no special-casing downstream.

use super::types::PayoutStructureEntry;

/// Default share of the field that gets paid (top 15%).
pub const DEFAULT_PERCENT_PAID: f64 = 15.0;

/// Geometric decay factor between consecutive places. 0.70 gives a recognisable
/// pro curve — e.g. ~46/32/22 for 3 places, ~59/41 for 2 — with a small but
/// non-zero min-cash tail for larger fields.
const DECAY: f64 = 0.70;

/// Number of paid places for a field of `entrants` paying the top `percent`.
pub fn paid_places(entrants: i32, percent: f64) -> i32 {
    if entrants <= 1 {
        return entrants.max(0);
    }
    let pct = percent.clamp(0.0, 100.0);
    let places = ((entrants as f64) * pct / 100.0).round() as i32;
    places.clamp(1, entrants)
}

/// Build a decaying payout structure (position → percentage) for a field of
/// `entrants` paying the top `percent`. Percentages sum to 100.00.
/// Returns an empty vec when there is no one to pay.
pub fn auto_payout_structure(entrants: i32, percent: f64) -> Vec<PayoutStructureEntry> {
    let places = paid_places(entrants, percent);
    if places <= 0 {
        return Vec::new();
    }

    // Geometric weights, normalised to percentages rounded to 2 decimals.
    let weights: Vec<f64> = (0..places).map(|i| DECAY.powi(i)).collect();
    let total_weight: f64 = weights.iter().sum();

    let mut entries: Vec<PayoutStructureEntry> = weights
        .iter()
        .enumerate()
        .map(|(i, w)| PayoutStructureEntry {
            position: (i + 1) as i32,
            percentage: round2(w / total_weight * 100.0),
        })
        .collect();

    // Absorb rounding drift into first place so the total is exactly 100.00.
    let sum: f64 = entries.iter().map(|e| e.percentage).sum();
    if let Some(first) = entries.first_mut() {
        first.percentage = round2(first.percentage + (100.0 - sum));
    }

    entries
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_scale_with_field_and_percent() {
        assert_eq!(paid_places(100, 15.0), 15);
        assert_eq!(paid_places(20, 15.0), 3);
        assert_eq!(paid_places(9, 15.0), 1); // round(1.35) = 1
        assert_eq!(paid_places(1, 15.0), 1); // heads-up edge: pay the winner
        assert_eq!(paid_places(0, 15.0), 0);
        assert_eq!(paid_places(50, 20.0), 10);
    }

    #[test]
    fn percentages_sum_to_100() {
        for entrants in [2, 3, 9, 20, 37, 100, 250] {
            for pct in [10.0, 15.0, 20.0] {
                let structure = auto_payout_structure(entrants, pct);
                let sum: f64 = structure.iter().map(|e| e.percentage).sum();
                assert!(
                    (sum - 100.0).abs() < 0.001,
                    "entrants={entrants} pct={pct}: sum={sum}"
                );
            }
        }
    }

    #[test]
    fn curve_is_strictly_decaying() {
        let structure = auto_payout_structure(100, 15.0);
        assert_eq!(structure.len(), 15);
        for pair in structure.windows(2) {
            assert!(
                pair[0].percentage >= pair[1].percentage,
                "expected non-increasing payouts, got {} then {}",
                pair[0].percentage,
                pair[1].percentage
            );
        }
        // Winner takes the largest single share.
        assert!(structure[0].percentage > structure[1].percentage);
    }

    #[test]
    fn winner_takes_all_when_one_place() {
        let structure = auto_payout_structure(8, 10.0); // round(0.8) = 1 place
        assert_eq!(structure.len(), 1);
        assert_eq!(structure[0].percentage, 100.0);
    }

    #[test]
    fn positions_are_one_indexed_and_contiguous() {
        let structure = auto_payout_structure(20, 15.0);
        for (i, entry) in structure.iter().enumerate() {
            assert_eq!(entry.position, (i + 1) as i32);
        }
    }
}
