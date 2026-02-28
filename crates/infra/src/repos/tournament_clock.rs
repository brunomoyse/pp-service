use crate::models::{TournamentClockRow, TournamentStructureRow};
use chrono::{Duration, Utc};
use sqlx::{PgPool, Result as SqlxResult};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClockStatus {
    Stopped,
    Running,
    Paused,
}

impl ClockStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClockStatus::Stopped => "stopped",
            ClockStatus::Running => "running",
            ClockStatus::Paused => "paused",
        }
    }
}

impl FromStr for ClockStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "stopped" => Ok(ClockStatus::Stopped),
            "running" => Ok(ClockStatus::Running),
            "paused" => Ok(ClockStatus::Paused),
            _ => Err(format!("Unknown clock status: {}", s)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TournamentStructureLevel {
    pub level_number: i32,
    pub small_blind: i32,
    pub big_blind: i32,
    pub ante: i32,
    pub duration_minutes: i32,
    pub is_break: bool,
    pub break_duration_minutes: Option<i32>,
}

/// Get tournament clock state
pub async fn get_clock(
    pool: &PgPool,
    tournament_id: Uuid,
) -> SqlxResult<Option<TournamentClockRow>> {
    sqlx::query_as::<_, TournamentClockRow>(
        "SELECT id, tournament_id, clock_status, current_level, level_started_at, level_end_time,
                pause_started_at, total_pause_duration, auto_advance, created_at, updated_at
         FROM tournament_clocks WHERE tournament_id = $1",
    )
    .bind(tournament_id)
    .fetch_optional(pool)
    .await
}

/// Initialize tournament clock
pub async fn create_clock(pool: &PgPool, tournament_id: Uuid) -> SqlxResult<TournamentClockRow> {
    sqlx::query_as::<_, TournamentClockRow>(
        "INSERT INTO tournament_clocks (tournament_id, clock_status, current_level)
         VALUES ($1, 'stopped', 1)
         RETURNING id, tournament_id, clock_status, current_level, level_started_at, level_end_time,
                   pause_started_at, total_pause_duration, auto_advance, created_at, updated_at",
    )
    .bind(tournament_id)
    .fetch_one(pool)
    .await
}

/// Start/resume tournament clock
pub async fn start_clock(
    pool: &PgPool,
    tournament_id: Uuid,
    manager_id: Option<Uuid>,
) -> SqlxResult<TournamentClockRow> {
    let now = Utc::now();

    // Get current structure to calculate end time
    let structure = get_current_structure(pool, tournament_id).await?;
    let level_duration = Duration::minutes(structure.duration_minutes as i64);
    let level_end_time = now + level_duration;

    let clock = sqlx::query_as::<_, TournamentClockRow>(
        "UPDATE tournament_clocks
         SET clock_status = 'running',
             level_started_at = $2,
             level_end_time = $3,
             pause_started_at = NULL,
             total_pause_duration = '0 seconds'
         WHERE tournament_id = $1
         RETURNING id, tournament_id, clock_status, current_level, level_started_at, level_end_time,
                   pause_started_at, total_pause_duration, auto_advance, created_at, updated_at",
    )
    .bind(tournament_id)
    .bind(now)
    .bind(level_end_time)
    .fetch_one(pool)
    .await?;

    // Log event
    log_event(
        pool,
        tournament_id,
        "start",
        Some(clock.current_level),
        manager_id,
        serde_json::json!({}),
    )
    .await?;

    Ok(clock)
}

/// Pause tournament clock
pub async fn pause_clock(
    pool: &PgPool,
    tournament_id: Uuid,
    manager_id: Option<Uuid>,
) -> SqlxResult<TournamentClockRow> {
    let now = Utc::now();

    let clock = sqlx::query_as::<_, TournamentClockRow>(
        "UPDATE tournament_clocks
         SET clock_status = 'paused',
             pause_started_at = $2
         WHERE tournament_id = $1
         RETURNING id, tournament_id, clock_status, current_level, level_started_at, level_end_time,
                   pause_started_at, total_pause_duration, auto_advance, created_at, updated_at",
    )
    .bind(tournament_id)
    .bind(now)
    .fetch_one(pool)
    .await?;

    // Log event
    log_event(
        pool,
        tournament_id,
        "pause",
        Some(clock.current_level),
        manager_id,
        serde_json::json!({}),
    )
    .await?;

    Ok(clock)
}

/// Resume tournament clock from pause
pub async fn resume_clock(
    pool: &PgPool,
    tournament_id: Uuid,
    manager_id: Option<Uuid>,
) -> SqlxResult<TournamentClockRow> {
    let now = Utc::now();

    let clock = sqlx::query_as::<_, TournamentClockRow>(
        "UPDATE tournament_clocks
         SET clock_status = 'running',
             total_pause_duration = total_pause_duration + (EXTRACT(EPOCH FROM ($2 - pause_started_at)) * INTERVAL '1 second'),
             level_end_time = level_end_time + (EXTRACT(EPOCH FROM ($2 - pause_started_at)) * INTERVAL '1 second'),
             pause_started_at = NULL
         WHERE tournament_id = $1
         RETURNING id, tournament_id, clock_status, current_level, level_started_at, level_end_time,
                   pause_started_at, total_pause_duration, auto_advance, created_at, updated_at"
    )
    .bind(tournament_id)
    .bind(now)
    .fetch_one(pool)
    .await?;

    // Log event
    log_event(
        pool,
        tournament_id,
        "resume",
        Some(clock.current_level),
        manager_id,
        serde_json::json!({}),
    )
    .await?;

    Ok(clock)
}

/// Advance to next level
pub async fn advance_level(
    pool: &PgPool,
    tournament_id: Uuid,
    auto: bool,
    manager_id: Option<Uuid>,
) -> SqlxResult<TournamentClockRow> {
    let now = Utc::now();

    // First increment the level
    sqlx::query(
        "UPDATE tournament_clocks SET current_level = current_level + 1 WHERE tournament_id = $1",
    )
    .bind(tournament_id)
    .execute(pool)
    .await?;

    // Get the new structure for timing
    let structure = get_current_structure(pool, tournament_id).await?;
    let level_duration = Duration::minutes(structure.duration_minutes as i64);
    let level_end_time = now + level_duration;

    // Update tournament clock, preserving any accumulated pause time
    let clock = sqlx::query_as::<_, TournamentClockRow>(
        "UPDATE tournament_clocks
         SET level_started_at = $2,
             level_end_time = $3,
             total_pause_duration = CASE
                 WHEN pause_started_at IS NOT NULL THEN
                     total_pause_duration + (EXTRACT(EPOCH FROM ($2 - pause_started_at)) * INTERVAL '1 second')
                 ELSE
                     total_pause_duration
             END,
             pause_started_at = NULL,
             clock_status = 'running'
         WHERE tournament_id = $1
         RETURNING id, tournament_id, clock_status, current_level, level_started_at, level_end_time,
                   pause_started_at, total_pause_duration, auto_advance, created_at, updated_at"
    )
    .bind(tournament_id)
    .bind(now)
    .bind(level_end_time)
    .fetch_one(pool)
    .await?;

    // Log event
    let event_type = if auto {
        "level_advance"
    } else {
        "manual_advance"
    };
    log_event(
        pool,
        tournament_id,
        event_type,
        Some(clock.current_level),
        manager_id,
        serde_json::json!({}),
    )
    .await?;

    Ok(clock)
}

/// Revert to previous level
pub async fn revert_level(
    pool: &PgPool,
    tournament_id: Uuid,
    manager_id: Option<Uuid>,
) -> SqlxResult<TournamentClockRow> {
    let now = Utc::now();

    // Check current level - don't allow going below level 1
    let current_clock = get_clock(pool, tournament_id)
        .await?
        .ok_or_else(|| sqlx::Error::RowNotFound)?;

    if current_clock.current_level <= 1 {
        return Err(sqlx::Error::RowNotFound);
    }

    // Decrement the level
    sqlx::query(
        "UPDATE tournament_clocks SET current_level = current_level - 1 WHERE tournament_id = $1",
    )
    .bind(tournament_id)
    .execute(pool)
    .await?;

    // Get the new structure for timing
    let structure = get_current_structure(pool, tournament_id).await?;
    let level_duration = Duration::minutes(structure.duration_minutes as i64);
    let level_end_time = now + level_duration;

    // Update tournament clock, preserving any accumulated pause time
    let clock = sqlx::query_as::<_, TournamentClockRow>(
        "UPDATE tournament_clocks
         SET level_started_at = $2,
             level_end_time = $3,
             total_pause_duration = CASE
                 WHEN pause_started_at IS NOT NULL THEN
                     total_pause_duration + (EXTRACT(EPOCH FROM ($2 - pause_started_at)) * INTERVAL '1 second')
                 ELSE
                     total_pause_duration
             END,
             pause_started_at = NULL,
             clock_status = 'running'
         WHERE tournament_id = $1
         RETURNING id, tournament_id, clock_status, current_level, level_started_at, level_end_time,
                   pause_started_at, total_pause_duration, auto_advance, created_at, updated_at"
    )
    .bind(tournament_id)
    .bind(now)
    .bind(level_end_time)
    .fetch_one(pool)
    .await?;

    // Log event
    log_event(
        pool,
        tournament_id,
        "manual_revert",
        Some(clock.current_level),
        manager_id,
        serde_json::json!({}),
    )
    .await?;

    Ok(clock)
}

/// Get current level structure
pub async fn get_current_structure(
    pool: &PgPool,
    tournament_id: Uuid,
) -> SqlxResult<TournamentStructureRow> {
    let clock = get_clock(pool, tournament_id)
        .await?
        .ok_or_else(|| sqlx::Error::RowNotFound)?;

    sqlx::query_as::<_, TournamentStructureRow>(
        "SELECT id, tournament_id, level_number, small_blind, big_blind, ante,
                duration_minutes, is_break, break_duration_minutes, created_at
         FROM tournament_structures
         WHERE tournament_id = $1 AND level_number = $2",
    )
    .bind(tournament_id)
    .bind(clock.current_level)
    .fetch_one(pool)
    .await
}

/// Get all structures for a tournament
pub async fn get_all_structures(
    pool: &PgPool,
    tournament_id: Uuid,
) -> SqlxResult<Vec<TournamentStructureRow>> {
    sqlx::query_as::<_, TournamentStructureRow>(
        "SELECT id, tournament_id, level_number, small_blind, big_blind, ante,
                duration_minutes, is_break, break_duration_minutes, created_at
         FROM tournament_structures
         WHERE tournament_id = $1
         ORDER BY level_number ASC",
    )
    .bind(tournament_id)
    .fetch_all(pool)
    .await
}

/// Get the next structure level directly (avoids fetching all structures)
pub async fn get_next_structure(
    pool: &PgPool,
    tournament_id: Uuid,
    current_level: i32,
) -> SqlxResult<Option<TournamentStructureRow>> {
    sqlx::query_as::<_, TournamentStructureRow>(
        "SELECT id, tournament_id, level_number, small_blind, big_blind, ante,
                duration_minutes, is_break, break_duration_minutes, created_at
         FROM tournament_structures
         WHERE tournament_id = $1 AND level_number = $2 + 1
         LIMIT 1",
    )
    .bind(tournament_id)
    .bind(current_level)
    .fetch_optional(pool)
    .await
}

/// Add structure level
pub async fn add_structure(
    pool: &PgPool,
    tournament_id: Uuid,
    level: TournamentStructureLevel,
) -> SqlxResult<TournamentStructureRow> {
    sqlx::query_as::<_, TournamentStructureRow>(
        "INSERT INTO tournament_structures
         (tournament_id, level_number, small_blind, big_blind, ante, duration_minutes, is_break, break_duration_minutes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, tournament_id, level_number, small_blind, big_blind, ante,
                   duration_minutes, is_break, break_duration_minutes, created_at"
    )
    .bind(tournament_id)
    .bind(level.level_number)
    .bind(level.small_blind)
    .bind(level.big_blind)
    .bind(level.ante)
    .bind(level.duration_minutes)
    .bind(level.is_break)
    .bind(level.break_duration_minutes)
    .fetch_one(pool)
    .await
}

/// Replace all structure levels for a tournament (delete existing + insert new)
pub async fn replace_structures(
    pool: &PgPool,
    tournament_id: Uuid,
    levels: Vec<TournamentStructureLevel>,
) -> SqlxResult<Vec<TournamentStructureRow>> {
    // Delete existing structures
    sqlx::query("DELETE FROM tournament_structures WHERE tournament_id = $1")
        .bind(tournament_id)
        .execute(pool)
        .await?;

    // Insert new structures
    let mut results = Vec::new();
    for level in levels {
        let row = add_structure(pool, tournament_id, level).await?;
        results.push(row);
    }
    Ok(results)
}

/// Log clock event into the unified activity log
async fn log_event(
    pool: &PgPool,
    tournament_id: Uuid,
    event_type: &str,
    level_number: Option<i32>,
    manager_id: Option<Uuid>,
    metadata: serde_json::Value,
) -> SqlxResult<()> {
    let merged_metadata = if let Some(level) = level_number {
        let mut m = metadata;
        m.as_object_mut()
            .map(|obj| obj.insert("level_number".to_string(), serde_json::json!(level)));
        m
    } else {
        metadata
    };

    crate::repos::activity_log::log_activity(
        pool,
        tournament_id,
        "clock",
        event_type,
        manager_id,
        None,
        merged_metadata,
    )
    .await?;

    Ok(())
}

/// Get tournaments that need level advancement (only if next level exists)
pub async fn get_tournaments_to_advance(pool: &PgPool) -> SqlxResult<Vec<Uuid>> {
    let now = Utc::now();

    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT tc.tournament_id FROM tournament_clocks tc
         WHERE tc.clock_status = 'running'
           AND tc.auto_advance = true
           AND tc.level_end_time IS NOT NULL
           AND tc.level_end_time <= $1
           AND EXISTS (
               SELECT 1 FROM tournament_structures ts
               WHERE ts.tournament_id = tc.tournament_id
                 AND ts.level_number = tc.current_level + 1
           )",
    )
    .bind(now)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Get tournaments at final level that need to be stopped
pub async fn get_tournaments_at_final_level(pool: &PgPool) -> SqlxResult<Vec<Uuid>> {
    let now = Utc::now();

    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT tc.tournament_id FROM tournament_clocks tc
         WHERE tc.clock_status = 'running'
           AND tc.auto_advance = true
           AND tc.level_end_time IS NOT NULL
           AND tc.level_end_time <= $1
           AND NOT EXISTS (
               SELECT 1 FROM tournament_structures ts
               WHERE ts.tournament_id = tc.tournament_id
                 AND ts.level_number = tc.current_level + 1
           )",
    )
    .bind(now)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Stop clock when tournament reaches final level
pub async fn stop_clock_final_level(
    pool: &PgPool,
    tournament_id: Uuid,
) -> SqlxResult<TournamentClockRow> {
    let clock = sqlx::query_as::<_, TournamentClockRow>(
        "UPDATE tournament_clocks
         SET clock_status = 'stopped',
             auto_advance = false
         WHERE tournament_id = $1
         RETURNING id, tournament_id, clock_status, current_level, level_started_at, level_end_time,
                   pause_started_at, total_pause_duration, auto_advance, created_at, updated_at",
    )
    .bind(tournament_id)
    .fetch_one(pool)
    .await?;

    // Log event
    log_event(
        pool,
        tournament_id,
        "final_level_complete",
        Some(clock.current_level),
        None,
        serde_json::json!({"reason": "No more levels in structure"}),
    )
    .await?;

    Ok(clock)
}
