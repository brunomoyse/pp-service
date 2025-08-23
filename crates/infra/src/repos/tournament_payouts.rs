use sqlx::{PgPool, Result};
use uuid::Uuid;

use crate::models::TournamentPayoutRow;

pub struct TournamentPayoutRepo {
    db: PgPool,
}

impl TournamentPayoutRepo {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Get payout structure for a tournament
    pub async fn get_by_tournament(
        &self,
        tournament_id: Uuid,
    ) -> Result<Option<TournamentPayoutRow>> {
        let row = sqlx::query_as::<_, TournamentPayoutRow>(
            r#"
            SELECT id, tournament_id, template_id, player_count, 
                   total_prize_pool, payout_positions, created_at, updated_at
            FROM tournament_payouts
            WHERE tournament_id = $1
            "#,
        )
        .bind(tournament_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    /// Get payout structure by ID
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<TournamentPayoutRow>> {
        let row = sqlx::query_as::<_, TournamentPayoutRow>(
            r#"
            SELECT id, tournament_id, template_id, player_count, 
                   total_prize_pool, payout_positions, created_at, updated_at
            FROM tournament_payouts
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    /// Manually recalculate payouts for a tournament (useful if template changes)
    pub async fn recalculate(&self, tournament_id: Uuid) -> Result<Option<TournamentPayoutRow>> {
        // Delete existing payout
        sqlx::query("DELETE FROM tournament_payouts WHERE tournament_id = $1")
            .bind(tournament_id)
            .execute(&self.db)
            .await?;

        // Trigger recalculation by updating tournament status
        // This is a workaround to trigger the SQL function
        sqlx::query(
            r#"
            UPDATE tournaments 
            SET live_status = 'in_progress', updated_at = NOW() 
            WHERE id = $1 AND live_status = 'in_progress'
            "#,
        )
        .bind(tournament_id)
        .execute(&self.db)
        .await?;

        // Return the new payout
        self.get_by_tournament(tournament_id).await
    }

    /// Update payout positions (for manual adjustments)
    pub async fn update_positions(
        &self,
        tournament_id: Uuid,
        payout_positions: serde_json::Value,
    ) -> Result<TournamentPayoutRow> {
        let row = sqlx::query_as::<_, TournamentPayoutRow>(
            r#"
            UPDATE tournament_payouts
            SET payout_positions = $2, updated_at = NOW()
            WHERE tournament_id = $1
            RETURNING id, tournament_id, template_id, player_count, 
                      total_prize_pool, payout_positions, created_at, updated_at
            "#,
        )
        .bind(tournament_id)
        .bind(payout_positions)
        .fetch_one(&self.db)
        .await?;

        Ok(row)
    }

    /// Delete payout structure for a tournament
    pub async fn delete(&self, tournament_id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM tournament_payouts WHERE tournament_id = $1")
            .bind(tournament_id)
            .execute(&self.db)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
