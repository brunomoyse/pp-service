use crate::{
    db::Db,
    models::{TournamentClockRow, TournamentStructureRow},
};
use chrono::{Duration, Utc};
use sqlx::Result as SqlxResult;
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

#[derive(Debug, Clone)]
pub struct TournamentClockRepo {
    pub pool: Db,
}

impl TournamentClockRepo {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }

    /// Get tournament clock state
    pub async fn get_clock(&self, tournament_id: Uuid) -> SqlxResult<Option<TournamentClockRow>> {
        sqlx::query_as::<_, TournamentClockRow>(
            "SELECT id, tournament_id, clock_status, current_level, level_started_at, level_end_time, 
                    pause_started_at, total_pause_duration, auto_advance, created_at, updated_at
             FROM tournament_clocks WHERE tournament_id = $1"
        )
        .bind(tournament_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Initialize tournament clock
    pub async fn create_clock(&self, tournament_id: Uuid) -> SqlxResult<TournamentClockRow> {
        let clock = sqlx::query_as::<_, TournamentClockRow>(
            "INSERT INTO tournament_clocks (tournament_id, clock_status, current_level)
             VALUES ($1, 'stopped', 1)
             RETURNING id, tournament_id, clock_status, current_level, level_started_at, level_end_time,
                       pause_started_at, total_pause_duration, auto_advance, created_at, updated_at"
        )
        .bind(tournament_id)
        .fetch_one(&self.pool)
        .await?;

        // Get the first level structure to sync with tournament_state
        if let Ok(structure) = self.get_current_structure(tournament_id).await {
            // Ensure tournament_state is synced with initial clock level
            sqlx::query(
                "INSERT INTO tournament_state (
                    tournament_id, current_level, current_small_blind, 
                    current_big_blind, current_ante, level_duration_minutes
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (tournament_id)
                DO UPDATE SET
                    current_level = $2,
                    current_small_blind = $3,
                    current_big_blind = $4,
                    current_ante = $5,
                    level_duration_minutes = $6,
                    updated_at = NOW()",
            )
            .bind(tournament_id)
            .bind(1i32) // Starting at level 1
            .bind(structure.small_blind)
            .bind(structure.big_blind)
            .bind(structure.ante)
            .bind(structure.duration_minutes)
            .execute(&self.pool)
            .await?;
        }

        Ok(clock)
    }

    /// Start/resume tournament clock
    pub async fn start_clock(
        &self,
        tournament_id: Uuid,
        manager_id: Option<Uuid>,
    ) -> SqlxResult<TournamentClockRow> {
        let now = Utc::now();

        // Get current structure to calculate end time
        let structure = self.get_current_structure(tournament_id).await?;
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
                       pause_started_at, total_pause_duration, auto_advance, created_at, updated_at"
        )
        .bind(tournament_id)
        .bind(now)
        .bind(level_end_time)
        .fetch_one(&self.pool)
        .await?;

        // Log event
        self.log_event(
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
        &self,
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
                       pause_started_at, total_pause_duration, auto_advance, created_at, updated_at"
        )
        .bind(tournament_id)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        // Log event
        self.log_event(
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
        &self,
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
        .fetch_one(&self.pool)
        .await?;

        // Log event
        self.log_event(
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
        &self,
        tournament_id: Uuid,
        auto: bool,
        manager_id: Option<Uuid>,
    ) -> SqlxResult<TournamentClockRow> {
        let now = Utc::now();

        // First increment the level
        sqlx::query(
            "UPDATE tournament_clocks SET current_level = current_level + 1 WHERE tournament_id = $1"
        )
        .bind(tournament_id)
        .execute(&self.pool)
        .await?;

        // Get the new structure for timing
        let structure = self.get_current_structure(tournament_id).await?;
        let level_duration = Duration::minutes(structure.duration_minutes as i64);
        let level_end_time = now + level_duration;

        let clock = sqlx::query_as::<_, TournamentClockRow>(
            "UPDATE tournament_clocks 
             SET level_started_at = $2,
                 level_end_time = $3,
                 total_pause_duration = '0 seconds',
                 pause_started_at = NULL
             WHERE tournament_id = $1
             RETURNING id, tournament_id, clock_status, current_level, level_started_at, level_end_time,
                       pause_started_at, total_pause_duration, auto_advance, created_at, updated_at"
        )
        .bind(tournament_id)
        .bind(now)
        .bind(level_end_time)
        .fetch_one(&self.pool)
        .await?;

        // Sync tournament_state table with the new level and structure values
        sqlx::query(
            "UPDATE tournament_state 
             SET current_level = $2,
                 current_small_blind = $3,
                 current_big_blind = $4,
                 current_ante = $5,
                 level_started_at = $6,
                 level_duration_minutes = $7,
                 updated_at = NOW()
             WHERE tournament_id = $1",
        )
        .bind(tournament_id)
        .bind(clock.current_level)
        .bind(structure.small_blind)
        .bind(structure.big_blind)
        .bind(structure.ante)
        .bind(now)
        .bind(structure.duration_minutes)
        .execute(&self.pool)
        .await?;

        // Log event
        let event_type = if auto {
            "level_advance"
        } else {
            "manual_advance"
        };
        self.log_event(
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
        &self,
        tournament_id: Uuid,
        manager_id: Option<Uuid>,
    ) -> SqlxResult<TournamentClockRow> {
        let now = Utc::now();

        // Check current level - don't allow going below level 1
        let current_clock = self
            .get_clock(tournament_id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        if current_clock.current_level <= 1 {
            return Err(sqlx::Error::RowNotFound);
        }

        // Decrement the level
        sqlx::query(
            "UPDATE tournament_clocks SET current_level = current_level - 1 WHERE tournament_id = $1"
        )
        .bind(tournament_id)
        .execute(&self.pool)
        .await?;

        // Get the new structure for timing
        let structure = self.get_current_structure(tournament_id).await?;
        let level_duration = Duration::minutes(structure.duration_minutes as i64);
        let level_end_time = now + level_duration;

        let clock = sqlx::query_as::<_, TournamentClockRow>(
            "UPDATE tournament_clocks 
             SET level_started_at = $2,
                 level_end_time = $3,
                 total_pause_duration = '0 seconds',
                 pause_started_at = NULL
             WHERE tournament_id = $1
             RETURNING id, tournament_id, clock_status, current_level, level_started_at, level_end_time,
                       pause_started_at, total_pause_duration, auto_advance, created_at, updated_at"
        )
        .bind(tournament_id)
        .bind(now)
        .bind(level_end_time)
        .fetch_one(&self.pool)
        .await?;

        // Sync tournament_state table with the reverted level and structure values
        sqlx::query(
            "UPDATE tournament_state 
             SET current_level = $2,
                 current_small_blind = $3,
                 current_big_blind = $4,
                 current_ante = $5,
                 level_started_at = $6,
                 level_duration_minutes = $7,
                 updated_at = NOW()
             WHERE tournament_id = $1",
        )
        .bind(tournament_id)
        .bind(clock.current_level)
        .bind(structure.small_blind)
        .bind(structure.big_blind)
        .bind(structure.ante)
        .bind(now)
        .bind(structure.duration_minutes)
        .execute(&self.pool)
        .await?;

        // Log event
        self.log_event(
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
        &self,
        tournament_id: Uuid,
    ) -> SqlxResult<TournamentStructureRow> {
        let clock = self
            .get_clock(tournament_id)
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
        .fetch_one(&self.pool)
        .await
    }

    /// Get all structures for a tournament
    pub async fn get_all_structures(
        &self,
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
        .fetch_all(&self.pool)
        .await
    }

    /// Add structure level
    pub async fn add_structure(
        &self,
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
        .fetch_one(&self.pool)
        .await
    }

    /// Log clock event
    async fn log_event(
        &self,
        tournament_id: Uuid,
        event_type: &str,
        level_number: Option<i32>,
        manager_id: Option<Uuid>,
        metadata: serde_json::Value,
    ) -> SqlxResult<()> {
        sqlx::query(
            "INSERT INTO tournament_clock_events 
             (tournament_id, event_type, level_number, manager_id, metadata)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(tournament_id)
        .bind(event_type)
        .bind(level_number)
        .bind(manager_id)
        .bind(metadata)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get tournaments that need level advancement
    pub async fn get_tournaments_to_advance(&self) -> SqlxResult<Vec<Uuid>> {
        let now = Utc::now();

        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT tournament_id FROM tournament_clocks 
             WHERE clock_status = 'running' 
               AND auto_advance = true 
               AND level_end_time IS NOT NULL 
               AND level_end_time <= $1",
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}
