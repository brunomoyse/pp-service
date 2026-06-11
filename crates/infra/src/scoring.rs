//! Tournament scoring.
//!
//! The default (club-wide) formula is the authoritative one stored on
//! `tournament_results.points`:
//!
//! ```text
//! points = min(60, round(3 * (sqrt(field_size) / sqrt(rank)) * (log10(buy_in_eur) + 1) + 2))
//! ```
//!
//! Leagues (configurable leaderboards) parameterize this shape via
//! [`ScoringFormula`] and compute points on read with [`event_points_with`].
//! [`ScoringFormula::default`] reproduces the formula above exactly.

use serde::{Deserialize, Serialize};

/// Shape of the per-position factor in the scoring formula.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PositionCurve {
    /// `1 / sqrt(rank)` — the default; gentle decay rewarding deep runs.
    #[default]
    Sqrt,
    /// `1 / rank` — steeper, top-heavy.
    Harmonic,
    /// `(N - rank + 1) / N` — flat, linear by finishing position.
    Linear,
}

/// Parameterized scoring formula for a league.
///
/// `raw = base_points + field_multiplier * sqrt(N) *
///        (buyin_multiplier * log10(buy_in_eur) + 1) * position_factor(rank, N)`
///
/// then `points = 0` when `N < min_players`, else `clamp(round(raw), 0, cap)`.
///
/// [`Self::default`] reproduces the authoritative club formula.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScoringFormula {
    pub base_points: f64,
    pub field_multiplier: f64,
    pub buyin_multiplier: f64,
    #[serde(default)]
    pub position_curve: PositionCurve,
    /// Tournaments with fewer entrants than this award 0 points.
    pub min_players: u32,
    /// Maximum points a single result can score.
    pub cap: u32,
    /// When set, a player's leaderboard total counts only their best N results.
    /// `None` counts every result. (Applied at aggregation time, not here.)
    #[serde(default)]
    pub count_best_n: Option<u32>,
}

impl Default for ScoringFormula {
    fn default() -> Self {
        Self {
            base_points: 2.0,
            field_multiplier: 3.0,
            buyin_multiplier: 1.0,
            position_curve: PositionCurve::Sqrt,
            min_players: 1,
            cap: 60,
            count_best_n: None,
        }
    }
}

/// Points for one result under the authoritative (default) formula.
///
/// # Examples
///
/// ```
/// use infra::scoring::event_points;
///
/// assert_eq!(event_points(40, 1, 20.0), 46);
/// assert_eq!(event_points(50, 2, 30.0), 39);
/// assert_eq!(event_points(80, 9, 50.0), 26);
/// ```
pub fn event_points(field_size: u32, rank: u32, buy_in_eur: f64) -> u32 {
    event_points_with(&ScoringFormula::default(), field_size, rank, buy_in_eur)
}

/// Points for one result under an arbitrary [`ScoringFormula`].
///
/// Returns 0 for invalid inputs (`field_size < 1`, `rank` out of range,
/// `buy_in_eur <= 0`) or when `field_size < formula.min_players`.
pub fn event_points_with(
    formula: &ScoringFormula,
    field_size: u32,
    rank: u32,
    buy_in_eur: f64,
) -> u32 {
    // Guardrails: validate inputs.
    if field_size < 1 || rank < 1 || rank > field_size || buy_in_eur <= 0.0 {
        return 0;
    }
    if field_size < formula.min_players {
        return 0;
    }

    let field_size_f = field_size as f64;
    let rank_f = rank as f64;

    let position_factor = match formula.position_curve {
        PositionCurve::Sqrt => 1.0 / rank_f.sqrt(),
        PositionCurve::Harmonic => 1.0 / rank_f,
        PositionCurve::Linear => (field_size_f - rank_f + 1.0) / field_size_f,
    };
    let buyin_factor = formula.buyin_multiplier * buy_in_eur.log10() + 1.0;

    let raw_points = formula.base_points
        + formula.field_multiplier * field_size_f.sqrt() * buyin_factor * position_factor;

    if raw_points <= 0.0 {
        return 0;
    }

    let rounded_points = raw_points.round() as u32;
    std::cmp::min(formula.cap, rounded_points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_points_basic_cases() {
        // Test cases from specification
        assert_eq!(event_points(40, 1, 20.0), 46);
        assert_eq!(event_points(50, 2, 30.0), 39);
        assert_eq!(event_points(80, 9, 50.0), 26);
    }

    #[test]
    fn test_event_points_edge_cases() {
        // Minimum valid inputs - let's calculate what this should actually be
        // field_size=1, rank=1, buy_in=0.01
        // 3 * (sqrt(1) / sqrt(1)) * (log10(0.01) + 1) + 2
        // = 3 * (1 / 1) * (-2 + 1) + 2
        // = 3 * 1 * (-1) + 2
        // = -3 + 2 = -1, rounded = 0 (because we can't have negative points)
        let small_points = event_points(1, 1, 0.01);
        assert_eq!(small_points, 0); // Should be 0 due to negative calculation

        // Better minimum case
        assert_eq!(event_points(1, 1, 1.0), 5); // log10(1) = 0, so 3*1*1 + 2 = 5

        // Large tournament, first place
        assert_eq!(event_points(200, 1, 100.0), 60); // Should cap at 60

        // Last place in small tournament
        let last_place_points = event_points(10, 10, 10.0);
        assert!(last_place_points > 0); // Should be positive
    }

    #[test]
    fn test_event_points_invalid_inputs() {
        // Invalid field size
        assert_eq!(event_points(0, 1, 10.0), 0);

        // Invalid rank (0)
        assert_eq!(event_points(10, 0, 10.0), 0);

        // Invalid rank (greater than field size)
        assert_eq!(event_points(10, 11, 10.0), 0);

        // Invalid buy-in (0)
        assert_eq!(event_points(10, 1, 0.0), 0);

        // Invalid buy-in (negative)
        assert_eq!(event_points(10, 1, -5.0), 0);
    }

    #[test]
    fn test_event_points_formula_components() {
        // Test that winners get more points than non-winners
        let winner_points = event_points(50, 1, 25.0);
        let second_points = event_points(50, 2, 25.0);
        let tenth_points = event_points(50, 10, 25.0);

        assert!(winner_points > second_points);
        assert!(second_points > tenth_points);

        // Test that larger field sizes generally give more points for same position
        let small_field = event_points(20, 1, 25.0);
        let large_field = event_points(100, 1, 25.0);

        assert!(large_field > small_field);

        // Test that higher buy-ins give more points for same position/field
        let low_buyin = event_points(50, 1, 10.0);
        let high_buyin = event_points(50, 1, 100.0);

        assert!(high_buyin > low_buyin);
    }

    #[test]
    fn test_event_points_cap() {
        // Test that points are capped at 60
        let max_points = event_points(1000, 1, 1000.0);
        assert_eq!(max_points, 60);
    }

    #[test]
    fn test_default_formula_matches_legacy() {
        // The parameterized default must reproduce the authoritative formula.
        let f = ScoringFormula::default();
        assert_eq!(event_points_with(&f, 40, 1, 20.0), 46);
        assert_eq!(event_points_with(&f, 50, 2, 30.0), 39);
        assert_eq!(event_points_with(&f, 80, 9, 50.0), 26);
    }

    #[test]
    fn test_position_curves_differ() {
        // For a non-winner, harmonic decays faster than sqrt, linear is flattest.
        let sqrt = ScoringFormula {
            position_curve: PositionCurve::Sqrt,
            ..Default::default()
        };
        let harmonic = ScoringFormula {
            position_curve: PositionCurve::Harmonic,
            ..Default::default()
        };
        let linear = ScoringFormula {
            position_curve: PositionCurve::Linear,
            ..Default::default()
        };

        let p_sqrt = event_points_with(&sqrt, 50, 5, 30.0);
        let p_harm = event_points_with(&harmonic, 50, 5, 30.0);
        assert!(p_harm < p_sqrt, "harmonic should decay faster than sqrt");

        // Linear keeps mid-pack finishers higher than harmonic for a deep field.
        let p_lin = event_points_with(&linear, 50, 5, 30.0);
        assert!(p_lin > p_harm);
    }

    #[test]
    fn test_min_players_gate() {
        let f = ScoringFormula {
            min_players: 10,
            ..Default::default()
        };
        assert_eq!(event_points_with(&f, 9, 1, 20.0), 0); // below threshold
        assert!(event_points_with(&f, 10, 1, 20.0) > 0); // at threshold
    }

    #[test]
    fn test_cap_is_configurable() {
        let high = ScoringFormula {
            cap: 100,
            ..Default::default()
        };
        // A huge field/buy-in that the default would clamp to 60.
        assert!(event_points_with(&high, 1000, 1, 1000.0) > 60);
        let low = ScoringFormula {
            cap: 10,
            ..Default::default()
        };
        assert_eq!(event_points_with(&low, 1000, 1, 1000.0), 10);
    }

    #[test]
    fn test_multipliers_scale() {
        // Doubling the field weight raises a 40-field/€20 winner above the default's 46.
        let f = ScoringFormula {
            field_multiplier: 6.0,
            ..Default::default()
        };
        assert!(event_points_with(&f, 40, 1, 20.0) > 46);
    }
}
