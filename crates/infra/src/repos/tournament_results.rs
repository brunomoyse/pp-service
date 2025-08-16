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
pub struct LeaderboardEntry {
    pub user_id: Uuid,
    pub username: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
    pub email: String,
    pub phone: Option<String>,
    pub is_active: bool,
    pub role: Option<String>,
    pub total_tournaments: i32,
    pub total_buy_ins: i32,  // Total amount spent (cents)
    pub total_winnings: i32, // Total amount won (cents)
    pub net_profit: i32,     // winnings - buy_ins (cents)
    pub total_itm: i32,      // Number of tournaments where player finished in the money
    pub itm_percentage: f64, // (total_itm / total_tournaments) * 100
    pub roi_percentage: f64, // ((total_winnings - total_buy_ins) / total_buy_ins) * 100
    pub average_finish: f64, // Average finishing position
    pub first_places: i32,   // Number of first place finishes
    pub final_tables: i32,   // Number of final table finishes (typically top 8-10)
    pub points: f64,         // Calculated leaderboard points
}

#[derive(Debug, Clone)]
pub enum LeaderboardPeriod {
    AllTime,
    LastYear,
    Last6Months,
    Last30Days,
    Last7Days,
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
        // Insert tournament result without calculating points
        // Points will be calculated automatically when tournament status changes to 'finished'
        let row = sqlx::query_as::<_, TournamentResultRow>(
            r#"
            INSERT INTO tournament_results (tournament_id, user_id, final_position, prize_cents, points, notes)
            VALUES ($1, $2, $3, $4, 0, $5)
            RETURNING id, tournament_id, user_id, final_position, prize_cents, points, notes, created_at, updated_at
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
            SELECT id, tournament_id, user_id, final_position, prize_cents, points, notes, created_at, updated_at
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
            SELECT id, tournament_id, user_id, final_position, prize_cents, points, notes, created_at, updated_at
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

    pub async fn get_user_recent_results(
        &self,
        user_id: Uuid,
        limit: i64,
    ) -> Result<Vec<TournamentResultRow>> {
        let rows = sqlx::query_as::<_, TournamentResultRow>(
            r#"
            SELECT tr.id, tr.tournament_id, tr.user_id, tr.final_position, tr.prize_cents, tr.points, tr.notes, tr.created_at, tr.updated_at
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

    pub async fn get_user_statistics(
        &self,
        user_id: Uuid,
        days_back: i32,
    ) -> Result<UserStatistics> {
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
            "#,
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
            "#,
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

    pub async fn update(
        &self,
        id: Uuid,
        data: CreateTournamentResult,
    ) -> Result<TournamentResultRow> {
        // Update tournament result without recalculating points
        // Points will be recalculated automatically when tournament status changes to 'finished'
        let row = sqlx::query_as::<_, TournamentResultRow>(
            r#"
            UPDATE tournament_results 
            SET tournament_id = $2, user_id = $3, final_position = $4, prize_cents = $5, notes = $6, updated_at = NOW()
            WHERE id = $1
            RETURNING id, tournament_id, user_id, final_position, prize_cents, points, notes, created_at, updated_at
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

    /// Calculate comprehensive leaderboard with points system
    pub async fn get_leaderboard(
        &self,
        period: LeaderboardPeriod,
        limit: Option<i32>,
        club_id: Option<Uuid>,
    ) -> Result<Vec<LeaderboardEntry>> {
        let (date_filter, _params_count) = match period {
            LeaderboardPeriod::AllTime => ("".to_string(), 0),
            LeaderboardPeriod::LastYear => (
                "AND t.start_time >= NOW() - INTERVAL '1 year'".to_string(),
                0,
            ),
            LeaderboardPeriod::Last6Months => (
                "AND t.start_time >= NOW() - INTERVAL '6 months'".to_string(),
                0,
            ),
            LeaderboardPeriod::Last30Days => (
                "AND t.start_time >= NOW() - INTERVAL '30 days'".to_string(),
                0,
            ),
            LeaderboardPeriod::Last7Days => (
                "AND t.start_time >= NOW() - INTERVAL '7 days'".to_string(),
                0,
            ),
        };

        let club_filter = match club_id {
            Some(_) => "AND t.club_id = $1".to_string(),
            None => "".to_string(),
        };

        let limit_clause = match limit {
            Some(l) => format!("LIMIT {}", l.clamp(1, 500)),
            None => "LIMIT 100".to_string(),
        };

        let query = format!(
            r#"
            WITH player_stats AS (
                SELECT 
                    u.id as user_id,
                    u.username,
                    u.first_name,
                    u.last_name,
                    u.email,
                    u.phone,
                    u.is_active,
                    u.role,
                    COUNT(DISTINCT reg.tournament_id) as total_tournaments,
                    COALESCE(SUM(t.buy_in_cents), 0) as total_buy_ins,
                    COALESCE(SUM(tr.prize_cents), 0) as total_winnings,
                    COUNT(tr.id) as total_itm,
                    COALESCE(AVG(tr.final_position::float), 0) as average_finish,
                    SUM(CASE WHEN tr.final_position = 1 THEN 1 ELSE 0 END) as first_places,
                    SUM(CASE WHEN tr.final_position <= 9 THEN 1 ELSE 0 END) as final_tables,
                    COALESCE(SUM(tr.points), 0) as total_points
                FROM users u
                JOIN tournament_registrations reg ON u.id = reg.user_id
                JOIN tournaments t ON reg.tournament_id = t.id
                LEFT JOIN tournament_results tr ON u.id = tr.user_id AND t.id = tr.tournament_id
                WHERE u.role = 'player' AND u.is_active = true 
                    {} {}
                GROUP BY u.id, u.username, u.first_name, u.last_name, u.email, u.phone, u.is_active, u.role
                HAVING COUNT(DISTINCT reg.tournament_id) > 0
            )
            SELECT 
                user_id,
                username,
                first_name,
                last_name,
                email,
                phone,
                is_active,
                role,
                total_tournaments,
                total_buy_ins,
                total_winnings,
                (total_winnings - total_buy_ins) as net_profit,
                total_itm,
                CASE 
                    WHEN total_tournaments > 0 THEN ROUND(CAST((total_itm::float / total_tournaments::float) * 100.0 AS NUMERIC), 2)::double precision
                    ELSE 0.0 
                END as itm_percentage,
                CASE 
                    WHEN total_buy_ins > 0 THEN ROUND(CAST(((total_winnings - total_buy_ins)::float / total_buy_ins::float) * 100.0 AS NUMERIC), 2)::double precision
                    ELSE 0.0 
                END as roi_percentage,
                ROUND(CAST(average_finish AS NUMERIC), 2)::double precision as average_finish,
                first_places,
                final_tables,
                total_points,
                -- Use total_points as the leaderboard score (sum of individual event points)
                total_points as points
            FROM player_stats
            ORDER BY points DESC, total_winnings DESC, total_tournaments DESC
            {}
            "#,
            date_filter, club_filter, limit_clause
        );

        let mut query_builder = sqlx::query(&query);

        // Bind club_id parameter if provided
        if let Some(club_uuid) = club_id {
            query_builder = query_builder.bind(club_uuid);
        }

        let rows = query_builder.fetch_all(&self.db).await?;

        let mut leaderboard = Vec::new();

        for row in rows {
            let entry = LeaderboardEntry {
                user_id: row.try_get("user_id")?,
                username: row.try_get("username")?,
                first_name: row.try_get("first_name")?,
                last_name: row.try_get("last_name")?,
                email: row.try_get("email")?,
                phone: row.try_get("phone")?,
                is_active: row.try_get("is_active")?,
                role: row.try_get("role")?,
                total_tournaments: row.try_get::<i64, _>("total_tournaments")? as i32,
                total_buy_ins: row.try_get::<i64, _>("total_buy_ins")? as i32,
                total_winnings: row.try_get::<i64, _>("total_winnings")? as i32,
                net_profit: row.try_get::<i64, _>("net_profit")? as i32,
                total_itm: row.try_get::<i64, _>("total_itm")? as i32,
                itm_percentage: row.try_get::<f64, _>("itm_percentage")?,
                roi_percentage: row.try_get::<f64, _>("roi_percentage")?,
                average_finish: row.try_get::<f64, _>("average_finish")?,
                first_places: row.try_get::<i64, _>("first_places")? as i32,
                final_tables: row.try_get::<i64, _>("final_tables")? as i32,
                points: row.try_get::<i64, _>("points")? as f64,
            };
            leaderboard.push(entry);
        }

        Ok(leaderboard)
    }
}
