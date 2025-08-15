use sqlx::{PgPool, Result, Row};
use uuid::Uuid;

use crate::models::TournamentResultRow;

#[derive(Debug, Clone)]
pub struct UserStatistics {
    pub total_itm: i32,
    pub total_tournaments: i32,
    pub total_winnings: i32,
    pub total_buy_ins: i32,
    pub itm_percentage: f64,
    pub roi_percentage: f64,
}

#[derive(Debug, Clone)]
pub struct CreateTournamentResult {
    pub tournament_id: Uuid,
    pub user_id: Uuid,
    pub final_position: i32,
    pub prize_cents: i32,
    pub notes: Option<String>,
}

pub struct TournamentResultRepo {
    db: PgPool,
}

impl TournamentResultRepo {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn create(&self, data: CreateTournamentResult) -> Result<TournamentResultRow> {
        let row = sqlx::query_as::<_, TournamentResultRow>(
            r#"
            INSERT INTO tournament_results (tournament_id, user_id, final_position, prize_cents, notes)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, tournament_id, user_id, final_position, prize_cents, notes, created_at, updated_at
            "#
        )
        .bind(data.tournament_id)
        .bind(data.user_id)
        .bind(data.final_position)
        .bind(data.prize_cents)
        .bind(data.notes)
        .fetch_one(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<TournamentResultRow>> {
        let row = sqlx::query_as::<_, TournamentResultRow>(
            r#"
            SELECT id, tournament_id, user_id, final_position, prize_cents, notes, created_at, updated_at
            FROM tournament_results
            WHERE id = $1
            "#
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn get_by_tournament(&self, tournament_id: Uuid) -> Result<Vec<TournamentResultRow>> {
        let rows = sqlx::query_as::<_, TournamentResultRow>(
            r#"
            SELECT id, tournament_id, user_id, final_position, prize_cents, notes, created_at, updated_at
            FROM tournament_results
            WHERE tournament_id = $1
            ORDER BY final_position ASC
            "#
        )
        .bind(tournament_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }

    pub async fn get_user_recent_results(&self, user_id: Uuid, limit: i64) -> Result<Vec<TournamentResultRow>> {
        let rows = sqlx::query_as::<_, TournamentResultRow>(
            r#"
            SELECT tr.id, tr.tournament_id, tr.user_id, tr.final_position, tr.prize_cents, tr.notes, tr.created_at, tr.updated_at
            FROM tournament_results tr
            JOIN tournaments t ON tr.tournament_id = t.id
            WHERE tr.user_id = $1
            ORDER BY tr.created_at DESC
            LIMIT $2
            "#
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }

    pub async fn get_user_statistics(&self, user_id: Uuid, days_back: i32) -> Result<UserStatistics> {
        let cutoff_date = chrono::Utc::now() - chrono::Duration::days(days_back as i64);
        
        // Get ITM count and total prize money
        let itm_row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_itm,
                COALESCE(SUM(tr.prize_cents), 0) as total_winnings
            FROM tournament_results tr
            JOIN tournaments t ON tr.tournament_id = t.id
            WHERE tr.user_id = $1 
                AND tr.prize_cents > 0 
                AND tr.created_at >= $2
            "#
        )
        .bind(user_id)
        .bind(cutoff_date)
        .fetch_one(&self.db)
        .await?;

        // Get total tournament count and buy-ins for the same period
        let tournament_row = sqlx::query(
            r#"
            SELECT 
                COUNT(DISTINCT reg.tournament_id) as total_tournaments,
                COALESCE(SUM(t.buy_in_cents), 0) as total_buy_ins
            FROM tournament_registrations reg
            JOIN tournaments t ON reg.tournament_id = t.id
            WHERE reg.user_id = $1 
                AND reg.created_at >= $2
            "#
        )
        .bind(user_id)
        .bind(cutoff_date)
        .fetch_one(&self.db)
        .await?;

        let total_itm: i64 = itm_row.try_get("total_itm").unwrap_or(0);
        let total_winnings: i64 = itm_row.try_get("total_winnings").unwrap_or(0);
        let total_tournaments: i64 = tournament_row.try_get("total_tournaments").unwrap_or(0);
        let total_buy_ins: i64 = tournament_row.try_get("total_buy_ins").unwrap_or(0);

        let total_itm = total_itm as i32;
        let total_winnings = total_winnings as i32;
        let total_tournaments = total_tournaments as i32;
        let total_buy_ins = total_buy_ins as i32;

        // Calculate percentages
        let itm_percentage = if total_tournaments > 0 {
            (total_itm as f64 / total_tournaments as f64) * 100.0
        } else {
            0.0
        };

        let roi_percentage = if total_buy_ins > 0 {
            let profit = total_winnings - total_buy_ins;
            (profit as f64 / total_buy_ins as f64) * 100.0
        } else {
            0.0
        };

        Ok(UserStatistics {
            total_itm,
            total_tournaments,
            total_winnings,
            total_buy_ins,
            itm_percentage,
            roi_percentage,
        })
    }

    pub async fn update(&self, id: Uuid, data: CreateTournamentResult) -> Result<TournamentResultRow> {
        let row = sqlx::query_as::<_, TournamentResultRow>(
            r#"
            UPDATE tournament_results 
            SET tournament_id = $2, user_id = $3, final_position = $4, prize_cents = $5, notes = $6, updated_at = NOW()
            WHERE id = $1
            RETURNING id, tournament_id, user_id, final_position, prize_cents, notes, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(data.tournament_id)
        .bind(data.user_id)
        .bind(data.final_position)
        .bind(data.prize_cents)
        .bind(data.notes)
        .fetch_one(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM tournament_results WHERE id = $1")
            .bind(id)
            .execute(&self.db)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}