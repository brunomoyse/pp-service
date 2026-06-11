use async_graphql::{Enum, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};
use infra::scoring as sc;

/// Shape of the per-position factor in the scoring formula.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum PositionCurve {
    #[default]
    Sqrt,
    Harmonic,
    Linear,
}

impl From<sc::PositionCurve> for PositionCurve {
    fn from(c: sc::PositionCurve) -> Self {
        match c {
            sc::PositionCurve::Sqrt => PositionCurve::Sqrt,
            sc::PositionCurve::Harmonic => PositionCurve::Harmonic,
            sc::PositionCurve::Linear => PositionCurve::Linear,
        }
    }
}

impl From<PositionCurve> for sc::PositionCurve {
    fn from(c: PositionCurve) -> Self {
        match c {
            PositionCurve::Sqrt => sc::PositionCurve::Sqrt,
            PositionCurve::Harmonic => sc::PositionCurve::Harmonic,
            PositionCurve::Linear => sc::PositionCurve::Linear,
        }
    }
}

/// How a league decides which tournaments count.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum MembershipMode {
    /// Every club tournament whose start time is within the league's period.
    AllInPeriod,
    /// Only tournaments explicitly tagged with this league.
    Tagged,
}

impl From<&str> for MembershipMode {
    fn from(s: &str) -> Self {
        match s {
            "tagged" => MembershipMode::Tagged,
            _ => MembershipMode::AllInPeriod,
        }
    }
}

impl From<MembershipMode> for String {
    fn from(m: MembershipMode) -> Self {
        match m {
            MembershipMode::AllInPeriod => "all_in_period".to_string(),
            MembershipMode::Tagged => "tagged".to_string(),
        }
    }
}

/// A league's scoring formula (see `infra::scoring`).
#[derive(SimpleObject, Clone)]
pub struct ScoringFormula {
    pub base_points: f64,
    pub field_multiplier: f64,
    pub buyin_multiplier: f64,
    pub position_curve: PositionCurve,
    pub min_players: i32,
    pub cap: i32,
    /// When set, only a player's best N results count toward their league total.
    pub count_best_n: Option<i32>,
}

impl From<sc::ScoringFormula> for ScoringFormula {
    fn from(f: sc::ScoringFormula) -> Self {
        Self {
            base_points: f.base_points,
            field_multiplier: f.field_multiplier,
            buyin_multiplier: f.buyin_multiplier,
            position_curve: f.position_curve.into(),
            min_players: f.min_players as i32,
            cap: f.cap as i32,
            count_best_n: f.count_best_n.map(|n| n as i32),
        }
    }
}

#[derive(InputObject, Clone)]
pub struct ScoringFormulaInput {
    pub base_points: f64,
    pub field_multiplier: f64,
    pub buyin_multiplier: f64,
    pub position_curve: PositionCurve,
    pub min_players: i32,
    pub cap: i32,
    pub count_best_n: Option<i32>,
}

impl From<ScoringFormulaInput> for sc::ScoringFormula {
    fn from(f: ScoringFormulaInput) -> Self {
        Self {
            base_points: f.base_points,
            field_multiplier: f.field_multiplier,
            buyin_multiplier: f.buyin_multiplier,
            position_curve: f.position_curve.into(),
            min_players: f.min_players.max(0) as u32,
            cap: f.cap.max(0) as u32,
            count_best_n: f.count_best_n.filter(|&n| n >= 1).map(|n| n as u32),
        }
    }
}

/// A configurable league.
#[derive(SimpleObject, Clone)]
pub struct LeaderboardConfig {
    pub id: ID,
    pub club_id: ID,
    pub name: String,
    pub formula: ScoringFormula,
    pub membership_mode: MembershipMode,
    pub period_start: Option<DateTime<Utc>>,
    pub period_end: Option<DateTime<Utc>>,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<infra::repos::leaderboard_configs::LeaderboardConfigRow> for LeaderboardConfig {
    fn from(row: infra::repos::leaderboard_configs::LeaderboardConfigRow) -> Self {
        // Defensive: a malformed stored formula falls back to the default.
        let formula: sc::ScoringFormula =
            serde_json::from_value(row.formula_params).unwrap_or_default();
        Self {
            id: row.id.into(),
            club_id: row.club_id.into(),
            name: row.name,
            formula: formula.into(),
            membership_mode: MembershipMode::from(row.membership_mode.as_str()),
            period_start: row.period_start,
            period_end: row.period_end,
            is_default: row.is_default,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// One audited manual point adjustment.
#[derive(SimpleObject, Clone)]
pub struct LeaderboardAdjustment {
    pub id: ID,
    pub config_id: ID,
    pub club_player_id: ID,
    pub points_delta: i32,
    pub reason: String,
    pub created_by: Option<ID>,
    pub created_at: DateTime<Utc>,
}

impl From<infra::repos::leaderboard_adjustments::LeaderboardAdjustmentRow>
    for LeaderboardAdjustment
{
    fn from(row: infra::repos::leaderboard_adjustments::LeaderboardAdjustmentRow) -> Self {
        Self {
            id: row.id.into(),
            config_id: row.config_id.into(),
            club_player_id: row.club_player_id.into(),
            points_delta: row.points_delta,
            reason: row.reason,
            created_by: row.created_by.map(|id| id.into()),
            created_at: row.created_at,
        }
    }
}

#[derive(InputObject)]
pub struct CreateLeaderboardConfigInput {
    pub club_id: ID,
    pub name: String,
    pub formula: ScoringFormulaInput,
    pub membership_mode: Option<MembershipMode>,
    pub period_start: Option<DateTime<Utc>>,
    pub period_end: Option<DateTime<Utc>>,
    pub is_default: Option<bool>,
}

#[derive(InputObject)]
pub struct UpdateLeaderboardConfigInput {
    pub id: ID,
    pub name: Option<String>,
    pub formula: Option<ScoringFormulaInput>,
    pub membership_mode: Option<MembershipMode>,
    pub period_start: Option<DateTime<Utc>>,
    pub period_end: Option<DateTime<Utc>>,
    pub is_default: Option<bool>,
}

#[derive(InputObject)]
pub struct AddLeaderboardAdjustmentInput {
    pub config_id: ID,
    pub club_player_id: ID,
    pub points_delta: i32,
    pub reason: String,
}

/// One sample row for the live scoring preview.
#[derive(InputObject)]
pub struct ScoringSampleInput {
    pub field_size: i32,
    pub rank: i32,
    pub buy_in_cents: i32,
}
