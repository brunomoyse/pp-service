use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc, Weekday};
use uuid::Uuid;

use crate::gql::error::GqlError;
use infra::db::Db;
use infra::models::SeasonRow;
use infra::repos::{quests as quests_repo, seasons as seasons_repo};

use super::quests::{self, QuestMetric};
use super::types::{HallOfFameEntry, QuestProgress, Season, SeasonPass};

/// XP granted per check-in counted toward a season pass.
const CHECK_IN_XP: i64 = 10;
/// XP that spans a single pass tier.
const XP_PER_TIER: i64 = 100;

/// The current Monday-anchored week: (week_start date, [start, end) instants, ISO week).
fn current_week(now: DateTime<Utc>) -> (NaiveDate, DateTime<Utc>, DateTime<Utc>, u32) {
    let today = now.date_naive();
    let week_start = today.week(Weekday::Mon).first_day();
    let start = week_start.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let end = start + Duration::days(7);
    (week_start, start, end, today.iso_week().week())
}

/// Compute a player's pass standing for a season. XP = attendance + quest XP,
/// all earned within the season window (constraint G1: never purchased).
pub async fn compute_pass(
    db: &Db,
    season: &SeasonRow,
    user_id: Uuid,
) -> Result<SeasonPass, GqlError> {
    let check_ins = seasons_repo::check_in_count_in_window(
        db,
        user_id,
        season.club_id,
        season.starts_at,
        season.ends_at,
    )
    .await?;
    let quest_xp = quests_repo::xp_in_window(db, user_id, season.starts_at, season.ends_at).await?;

    let xp = check_ins * CHECK_IN_XP + quest_xp;

    Ok(SeasonPass {
        season_id: season.id.into(),
        xp: xp as i32,
        tier: (xp / XP_PER_TIER) as i32,
        xp_into_tier: (xp % XP_PER_TIER) as i32,
        xp_per_tier: XP_PER_TIER as i32,
    })
}

async fn quest_metric_value(
    db: &Db,
    user_id: Uuid,
    metric: QuestMetric,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<i64, GqlError> {
    Ok(match metric {
        QuestMetric::CheckIns => quests_repo::check_in_count(db, user_id, start, end).await?,
        QuestMetric::DistinctClubs => quests_repo::distinct_clubs(db, user_id, start, end).await?,
    })
}

/// The three active quests for this week with the player's live progress.
pub async fn weekly_quests(db: &Db, user_id: Uuid) -> Result<Vec<QuestProgress>, GqlError> {
    let (week_start, start, end, iso_week) = current_week(Utc::now());
    let claimed: Vec<String> = quests_repo::completions_for_week(db, user_id, week_start)
        .await?
        .into_iter()
        .map(|c| c.quest_code)
        .collect();

    let mut out = Vec::with_capacity(3);
    for def in quests::weekly(iso_week) {
        let value = quest_metric_value(db, user_id, def.metric, start, end).await?;
        let progress = value.min(def.target);
        out.push(QuestProgress {
            code: def.code.to_string(),
            target: def.target as i32,
            progress: progress as i32,
            completed: value >= def.target,
            claimed: claimed.iter().any(|c| c == def.code),
            xp_reward: def.xp_reward,
        });
    }
    Ok(out)
}

/// Claim a completed weekly quest, awarding its XP (idempotent per week).
pub async fn claim_quest(db: &Db, user_id: Uuid, code: &str) -> Result<QuestProgress, GqlError> {
    let (week_start, start, end, iso_week) = current_week(Utc::now());

    // Must be one of this week's active quests.
    let def = quests::weekly(iso_week)
        .into_iter()
        .find(|q| q.code == code)
        .ok_or_else(|| GqlError::new("That quest is not active this week"))?;

    let value = quest_metric_value(db, user_id, def.metric, start, end).await?;
    if value < def.target {
        return Err(GqlError::new("Quest not yet completed"));
    }

    quests_repo::claim(db, user_id, def.code, week_start, def.xp_reward).await?;

    Ok(QuestProgress {
        code: def.code.to_string(),
        target: def.target as i32,
        progress: def.target as i32,
        completed: true,
        claimed: true,
        xp_reward: def.xp_reward,
    })
}

/// Champions of every finished season for a club, newest first.
pub async fn hall_of_fame(db: &Db, club_id: Uuid) -> Result<Vec<HallOfFameEntry>, GqlError> {
    let now = Utc::now();
    let finished = seasons_repo::list_finished_by_club(db, club_id, now).await?;

    let mut out = Vec::new();
    for season in finished {
        if let Some(champ) =
            seasons_repo::champion_for_window(db, club_id, season.starts_at, season.ends_at).await?
        {
            out.push(HallOfFameEntry {
                season_id: season.id.into(),
                season_name: season.name,
                ends_at: season.ends_at,
                champion_name: champ.champion_name,
                events: champ.events as i32,
            });
        }
    }
    Ok(out)
}

/// Convert a row to the GraphQL type, stamping `is_active` against now.
pub fn to_season(row: SeasonRow) -> Season {
    Season::from_row(row, Utc::now())
}
