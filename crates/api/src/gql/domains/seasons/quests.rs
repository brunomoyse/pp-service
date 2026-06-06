//! Weekly quest catalog and deterministic rotation.
//!
//! Quests live in code (not the DB): the catalog is static and the three active
//! quests for any week are derived from the ISO week number — no scheduled job,
//! and every server agrees on the same rotation. Progress is computed from the
//! `check_in` table; only completions are persisted (`quest_completion`).

/// What a quest measures, all within the current week's window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestMetric {
    /// Number of check-ins this week (any club).
    CheckIns,
    /// Distinct clubs checked in at this week.
    DistinctClubs,
}

#[derive(Debug, Clone, Copy)]
pub struct QuestDef {
    pub code: &'static str,
    pub metric: QuestMetric,
    pub target: i64,
    pub xp_reward: i32,
}

/// The full pool. Three are surfaced each week (see [`weekly`]). Codes are stable
/// — the client maps them to localized title/description strings.
pub const CATALOG: &[QuestDef] = &[
    QuestDef {
        code: "weekly_regular",
        metric: QuestMetric::CheckIns,
        target: 1,
        xp_reward: 20,
    },
    QuestDef {
        code: "double_up",
        metric: QuestMetric::CheckIns,
        target: 2,
        xp_reward: 40,
    },
    QuestDef {
        code: "triple_threat",
        metric: QuestMetric::CheckIns,
        target: 3,
        xp_reward: 70,
    },
    QuestDef {
        code: "club_hopper",
        metric: QuestMetric::DistinctClubs,
        target: 2,
        xp_reward: 50,
    },
    QuestDef {
        code: "explorer",
        metric: QuestMetric::DistinctClubs,
        target: 3,
        xp_reward: 90,
    },
];

pub fn find(code: &str) -> Option<&'static QuestDef> {
    CATALOG.iter().find(|q| q.code == code)
}

/// The three quests active for a given ISO week (1-based week number). Rotation
/// is a sliding window over the catalog, so the set shifts predictably each week.
pub fn weekly(iso_week: u32) -> [&'static QuestDef; 3] {
    let n = CATALOG.len();
    let base = (iso_week as usize) % n;
    [
        &CATALOG[base % n],
        &CATALOG[(base + 1) % n],
        &CATALOG[(base + 2) % n],
    ]
}
