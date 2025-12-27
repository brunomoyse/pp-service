use sqlx::{PgPool, Result};
use uuid::Uuid;

use crate::models::TournamentEntryRow;

#[derive(Debug, Clone)]
pub struct CreateTournamentEntry {
    pub tournament_id: Uuid,
    pub user_id: Uuid,
    pub entry_type: String,
    pub amount_cents: i32,
    pub chips_received: Option<i32>,
    pub recorded_by: Option<Uuid>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TournamentEntryStats {
    pub tournament_id: Uuid,
    pub total_entries: i64,
    pub total_amount_cents: i64,
    pub unique_players: i64,
    pub initial_count: i64,
    pub rebuy_count: i64,
    pub re_entry_count: i64,
    pub addon_count: i64,
}

pub struct TournamentEntryRepo {
    db: PgPool,
}

impl TournamentEntryRepo {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Create a new tournament entry (initial, rebuy, re-entry, or add-on)
    pub async fn create(&self, data: CreateTournamentEntry) -> Result<TournamentEntryRow> {
        let row = sqlx::query_as::<_, TournamentEntryRow>(
            r#"
            INSERT INTO tournament_entries (
                tournament_id, user_id, entry_type, amount_cents,
                chips_received, recorded_by, notes
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, tournament_id, user_id, entry_type, amount_cents,
                      chips_received, recorded_by, notes, created_at, updated_at
            "#,
        )
        .bind(data.tournament_id)
        .bind(data.user_id)
        .bind(data.entry_type)
        .bind(data.amount_cents)
        .bind(data.chips_received)
        .bind(data.recorded_by)
        .bind(data.notes)
        .fetch_one(&self.db)
        .await?;

        Ok(row)
    }

    /// Get entry by ID
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<TournamentEntryRow>> {
        let row = sqlx::query_as::<_, TournamentEntryRow>(
            r#"
            SELECT id, tournament_id, user_id, entry_type, amount_cents,
                   chips_received, recorded_by, notes, created_at, updated_at
            FROM tournament_entries
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    /// Get all entries for a tournament
    pub async fn get_by_tournament(&self, tournament_id: Uuid) -> Result<Vec<TournamentEntryRow>> {
        let rows = sqlx::query_as::<_, TournamentEntryRow>(
            r#"
            SELECT id, tournament_id, user_id, entry_type, amount_cents,
                   chips_received, recorded_by, notes, created_at, updated_at
            FROM tournament_entries
            WHERE tournament_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(tournament_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }

    /// Get entries for a specific user in a tournament
    pub async fn get_by_tournament_and_user(
        &self,
        tournament_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<TournamentEntryRow>> {
        let rows = sqlx::query_as::<_, TournamentEntryRow>(
            r#"
            SELECT id, tournament_id, user_id, entry_type, amount_cents,
                   chips_received, recorded_by, notes, created_at, updated_at
            FROM tournament_entries
            WHERE tournament_id = $1 AND user_id = $2
            ORDER BY created_at ASC
            "#,
        )
        .bind(tournament_id)
        .bind(user_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }

    /// Get aggregated entry statistics for a tournament
    pub async fn get_stats(&self, tournament_id: Uuid) -> Result<TournamentEntryStats> {
        let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64)>(
            r#"
            SELECT
                COUNT(*) as total_entries,
                COALESCE(SUM(amount_cents), 0) as total_amount_cents,
                COUNT(DISTINCT user_id) as unique_players,
                COUNT(*) FILTER (WHERE entry_type = 'initial') as initial_count,
                COUNT(*) FILTER (WHERE entry_type = 'rebuy') as rebuy_count,
                COUNT(*) FILTER (WHERE entry_type = 're_entry') as re_entry_count,
                COUNT(*) FILTER (WHERE entry_type = 'addon') as addon_count
            FROM tournament_entries
            WHERE tournament_id = $1
            "#,
        )
        .bind(tournament_id)
        .fetch_one(&self.db)
        .await?;

        Ok(TournamentEntryStats {
            tournament_id,
            total_entries: row.0,
            total_amount_cents: row.1,
            unique_players: row.2,
            initial_count: row.3,
            rebuy_count: row.4,
            re_entry_count: row.5,
            addon_count: row.6,
        })
    }

    /// Delete an entry (for corrections)
    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM tournament_entries WHERE id = $1")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get total prize pool for a tournament (sum of all entry amounts)
    pub async fn get_total_prize_pool(&self, tournament_id: Uuid) -> Result<i64> {
        let result: (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount_cents), 0) FROM tournament_entries WHERE tournament_id = $1",
        )
        .bind(tournament_id)
        .fetch_one(&self.db)
        .await?;
        Ok(result.0)
    }

    /// Apply early bird bonus to a specific player's initial entry
    /// Returns the updated entry if found, None otherwise
    pub async fn apply_early_bird_bonus(
        &self,
        tournament_id: Uuid,
        user_id: Uuid,
        bonus_chips: i32,
    ) -> Result<Option<TournamentEntryRow>> {
        let row = sqlx::query_as::<_, TournamentEntryRow>(
            r#"
            UPDATE tournament_entries
            SET chips_received = COALESCE(chips_received, 0) + $3,
                updated_at = NOW()
            WHERE tournament_id = $1
              AND user_id = $2
              AND entry_type = 'initial'
            RETURNING id, tournament_id, user_id, entry_type, amount_cents,
                      chips_received, recorded_by, notes, created_at, updated_at
            "#,
        )
        .bind(tournament_id)
        .bind(user_id)
        .bind(bonus_chips)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    /// Apply early bird bonus to all checked-in players' initial entries
    /// Used when tournament transitions to in_progress
    /// Returns the number of entries updated
    pub async fn apply_early_bird_bonus_bulk(
        &self,
        tournament_id: Uuid,
        bonus_chips: i32,
        eligible_user_ids: &[Uuid],
    ) -> Result<u64> {
        if eligible_user_ids.is_empty() {
            return Ok(0);
        }

        let result = sqlx::query(
            r#"
            UPDATE tournament_entries
            SET chips_received = COALESCE(chips_received, 0) + $2,
                updated_at = NOW()
            WHERE tournament_id = $1
              AND user_id = ANY($3::uuid[])
              AND entry_type = 'initial'
            "#,
        )
        .bind(tournament_id)
        .bind(bonus_chips)
        .bind(eligible_user_ids)
        .execute(&self.db)
        .await?;

        Ok(result.rows_affected())
    }
}
