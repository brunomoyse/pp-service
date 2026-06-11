use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, PgPool, Result, Row};
use uuid::Uuid;

use crate::models::TournamentResultRow;
use crate::scoring::{event_points_with, ScoringFormula};

const COLS: &str = "id, tournament_id, user_id, club_player_id, final_position, prize_cents, points, notes, created_at, updated_at";

#[derive(Debug, Clone)]
pub struct UserStatistics {
    pub total_itm: i32,
    pub total_tournaments: i32,
    pub total_winnings: i32,
    pub total_buy_ins: i32,
    pub itm_percentage: f64,
    pub roi_percentage: f64,
}

/// One leaderboard row. The roster entry (club_player) is the identity;
/// `user_id` and the user fields are present only when the player has an app
/// account. `display_name` always renders the player.
#[derive(Debug, Clone)]
pub struct LeaderboardEntry {
    pub club_player_id: Uuid,
    pub display_name: String,
    pub user_id: Option<Uuid>,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub is_active: Option<bool>,
    pub role: Option<String>,
    pub locale: Option<String>,
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

#[derive(Debug, Clone, Copy)]
pub enum LeaderboardPeriod {
    AllTime,
    LastYear,
    Last6Months,
    Last30Days,
    Last7Days,
}

#[derive(Debug, Clone, Default)]
pub struct CreateTournamentResult {
    pub tournament_id: Uuid,
    /// App user, when the player has an account. The link trigger stamps
    /// whichever of user_id / club_player_id is missing.
    pub user_id: Option<Uuid>,
    pub club_player_id: Option<Uuid>,
    pub final_position: i32,
    pub prize_cents: i32,
    pub notes: Option<String>,
}

pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    data: CreateTournamentResult,
) -> Result<TournamentResultRow> {
    let row = sqlx::query_as::<_, TournamentResultRow>(&format!(
        "INSERT INTO tournament_results (tournament_id, user_id, club_player_id, final_position, prize_cents, points, notes) \
         VALUES ($1, $2, $3, $4, $5, 0, $6) RETURNING {COLS}"
    ))
    .bind(data.tournament_id)
    .bind(data.user_id)
    .bind(data.club_player_id)
    .bind(data.final_position)
    .bind(data.prize_cents)
    .bind(data.notes)
    .fetch_one(executor)
    .await?;

    Ok(row)
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> Result<Option<TournamentResultRow>> {
    let row = sqlx::query_as::<_, TournamentResultRow>(&format!(
        "SELECT {COLS} FROM tournament_results WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

pub async fn list_by_tournament<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> Result<Vec<TournamentResultRow>> {
    let rows = sqlx::query_as::<_, TournamentResultRow>(&format!(
        "SELECT {COLS} FROM tournament_results WHERE tournament_id = $1 ORDER BY final_position ASC"
    ))
    .bind(tournament_id)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

pub async fn list_user_recent<'e>(
    executor: impl PgExecutor<'e>,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<TournamentResultRow>> {
    let rows = sqlx::query_as::<_, TournamentResultRow>(
        "SELECT tr.id, tr.tournament_id, tr.user_id, tr.club_player_id, tr.final_position, \
                tr.prize_cents, tr.points, tr.notes, tr.created_at, tr.updated_at \
         FROM tournament_results tr \
         JOIN tournaments t ON tr.tournament_id = t.id \
         WHERE tr.user_id = $1 \
         ORDER BY tr.created_at DESC LIMIT $2",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

/// Uses multiple queries so requires &PgPool
pub async fn get_user_statistics(
    pool: &PgPool,
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
    .fetch_one(pool)
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
    .fetch_one(pool)
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

pub async fn update<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    data: CreateTournamentResult,
) -> Result<TournamentResultRow> {
    let row = sqlx::query_as::<_, TournamentResultRow>(&format!(
        "UPDATE tournament_results \
         SET tournament_id = $2, final_position = $3, prize_cents = $4, notes = $5, updated_at = NOW() \
         WHERE id = $1 RETURNING {COLS}"
    ))
    .bind(id)
    .bind(data.tournament_id)
    .bind(data.final_position)
    .bind(data.prize_cents)
    .bind(data.notes)
    .fetch_one(executor)
    .await?;

    Ok(row)
}

pub async fn delete<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM tournament_results WHERE id = $1")
        .bind(id)
        .execute(executor)
        .await?;

    Ok(result.rows_affected() > 0)
}

fn period_filter(period: LeaderboardPeriod) -> &'static str {
    match period {
        LeaderboardPeriod::AllTime => "",
        LeaderboardPeriod::LastYear => "AND t.start_time >= NOW() - INTERVAL '1 year'",
        LeaderboardPeriod::Last6Months => "AND t.start_time >= NOW() - INTERVAL '6 months'",
        LeaderboardPeriod::Last30Days => "AND t.start_time >= NOW() - INTERVAL '30 days'",
        LeaderboardPeriod::Last7Days => "AND t.start_time >= NOW() - INTERVAL '7 days'",
    }
}

/// Comprehensive leaderboard keyed on the club roster, so account-less players
/// rank alongside app users. Uses dynamic SQL so requires &PgPool.
pub async fn get_leaderboard(
    pool: &PgPool,
    period: LeaderboardPeriod,
    limit: Option<i32>,
    offset: Option<i32>,
    club_id: Option<Uuid>,
    province: Option<String>,
) -> Result<Vec<LeaderboardEntry>> {
    let date_filter = period_filter(period);
    // Optional filters bind in declaration order; track the next placeholder.
    let mut next_param = 0;
    let club_filter = if club_id.is_some() {
        next_param += 1;
        format!("AND t.club_id = ${next_param}")
    } else {
        String::new()
    };
    let province_filter = if province.is_some() {
        next_param += 1;
        format!("AND t.club_id IN (SELECT id FROM clubs WHERE province = ${next_param})")
    } else {
        String::new()
    };

    let limit_value = limit.unwrap_or(100).clamp(1, 500);
    let offset_value = offset.unwrap_or(0).max(0);
    let limit_clause = format!("LIMIT {} OFFSET {}", limit_value, offset_value);

    // Account-less roster entries (u.id IS NULL) always count; app users only
    // when they are active players (managers/admins are staff, not ranked).
    let query = format!(
        r#"
        WITH player_stats AS (
            SELECT
                rp.id as club_player_id,
                rp.display_name,
                u.id as user_id,
                u.username, u.first_name, u.last_name, u.email, u.phone,
                u.is_active, u.role, u.locale,
                COUNT(DISTINCT reg.tournament_id) as total_tournaments,
                COALESCE(SUM(t.buy_in_cents), 0) as total_buy_ins,
                COALESCE(SUM(tr.prize_cents), 0) as total_winnings,
                COUNT(tr.id) as total_itm,
                COALESCE(AVG(tr.final_position::float), 0) as average_finish,
                SUM(CASE WHEN tr.final_position = 1 THEN 1 ELSE 0 END) as first_places,
                SUM(CASE WHEN tr.final_position <= 9 THEN 1 ELSE 0 END) as final_tables,
                COALESCE(SUM(tr.points), 0) as total_points
            FROM club_player rp
            LEFT JOIN users u ON u.id = rp.app_user_id
            JOIN tournament_registrations reg ON reg.club_player_id = rp.id
            JOIN tournaments t ON reg.tournament_id = t.id
            LEFT JOIN tournament_results tr ON tr.club_player_id = rp.id AND tr.tournament_id = t.id
            WHERE (u.id IS NULL OR (u.role = 'player' AND u.is_active = true))
                {} {} {}
            GROUP BY rp.id, rp.display_name, u.id, u.username, u.first_name, u.last_name,
                     u.email, u.phone, u.is_active, u.role, u.locale
            HAVING COUNT(DISTINCT reg.tournament_id) > 0
        )
        SELECT
            club_player_id,
            display_name,
            user_id,
            username, first_name, last_name, email, phone, is_active, role, locale,
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
            total_points as points
        FROM player_stats
        ORDER BY points DESC, total_winnings DESC, total_tournaments DESC
        {}
        "#,
        date_filter, club_filter, province_filter, limit_clause
    );

    let mut query_builder = sqlx::query(&query);
    if let Some(club_uuid) = club_id {
        query_builder = query_builder.bind(club_uuid);
    }
    if let Some(province) = province {
        query_builder = query_builder.bind(province);
    }

    let rows = query_builder.fetch_all(pool).await?;

    let mut leaderboard = Vec::new();
    for row in rows {
        let entry = LeaderboardEntry {
            club_player_id: row.try_get("club_player_id")?,
            display_name: row.try_get("display_name")?,
            user_id: row.try_get("user_id")?,
            username: row.try_get("username")?,
            first_name: row.try_get("first_name")?,
            last_name: row.try_get("last_name")?,
            email: row.try_get("email")?,
            phone: row.try_get("phone")?,
            is_active: row.try_get("is_active")?,
            role: row.try_get("role")?,
            locale: row.try_get("locale")?,
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

/// Count total leaderboard entries for pagination
pub async fn count_leaderboard(
    pool: &PgPool,
    period: LeaderboardPeriod,
    club_id: Option<Uuid>,
    province: Option<String>,
) -> Result<i64> {
    let date_filter = period_filter(period);
    let mut next_param = 0;
    let club_filter = if club_id.is_some() {
        next_param += 1;
        format!("AND t.club_id = ${next_param}")
    } else {
        String::new()
    };
    let province_filter = if province.is_some() {
        next_param += 1;
        format!("AND t.club_id IN (SELECT id FROM clubs WHERE province = ${next_param})")
    } else {
        String::new()
    };

    let query = format!(
        r#"
        SELECT COUNT(*) as total FROM (
            SELECT rp.id
            FROM club_player rp
            LEFT JOIN users u ON u.id = rp.app_user_id
            JOIN tournament_registrations reg ON reg.club_player_id = rp.id
            JOIN tournaments t ON reg.tournament_id = t.id
            WHERE (u.id IS NULL OR (u.role = 'player' AND u.is_active = true))
                {} {} {}
            GROUP BY rp.id
            HAVING COUNT(DISTINCT reg.tournament_id) > 0
        ) c
        "#,
        date_filter, club_filter, province_filter
    );

    let mut query_builder = sqlx::query_scalar::<_, i64>(&query);
    if let Some(club_uuid) = club_id {
        query_builder = query_builder.bind(club_uuid);
    }
    if let Some(province) = province {
        query_builder = query_builder.bind(province);
    }

    let count = query_builder.fetch_one(pool).await?;
    Ok(count)
}

/// Leaderboard for a configurable league: points are recomputed per result from
/// `formula`, capped to the player's best `count_best_n` results, plus audited
/// manual adjustments. Returns the requested page and the total number of ranked
/// players. Stats (tournaments, winnings, ITM, …) are aggregated over the
/// league's tournament set; points are the only thing that differs per league.
#[allow(clippy::too_many_arguments)]
pub async fn get_leaderboard_for_config(
    pool: &PgPool,
    config_id: Uuid,
    formula: &ScoringFormula,
    membership_mode: &str,
    club_id: Uuid,
    period_start: Option<DateTime<Utc>>,
    period_end: Option<DateTime<Utc>>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<(Vec<LeaderboardEntry>, i64)> {
    // Tournament-set filter shared by both queries. $1 club, $2 period_start,
    // $3 period_end, $4 config (tagged only). Bind in the same order each time.
    let tagged = membership_mode == "tagged";
    let mut tournament_filter = String::from(
        "t.club_id = $1 \
         AND ($2::timestamptz IS NULL OR t.start_time >= $2) \
         AND ($3::timestamptz IS NULL OR t.start_time <= $3)",
    );
    if tagged {
        tournament_filter.push_str(" AND t.leaderboard_config_id = $4");
    }

    // --- Query 1: per-player stats (everything except points) ---
    let stats_sql = format!(
        r#"
        SELECT
            rp.id as club_player_id,
            rp.display_name,
            u.id as user_id,
            u.username, u.first_name, u.last_name, u.email, u.phone,
            u.is_active, u.role, u.locale,
            COUNT(DISTINCT reg.tournament_id) as total_tournaments,
            COALESCE(SUM(t.buy_in_cents), 0) as total_buy_ins,
            COALESCE(SUM(tr.prize_cents), 0) as total_winnings,
            COUNT(tr.id) as total_itm,
            COALESCE(AVG(tr.final_position::float), 0) as average_finish,
            SUM(CASE WHEN tr.final_position = 1 THEN 1 ELSE 0 END) as first_places,
            SUM(CASE WHEN tr.final_position <= 9 THEN 1 ELSE 0 END) as final_tables
        FROM club_player rp
        LEFT JOIN users u ON u.id = rp.app_user_id
        JOIN tournament_registrations reg ON reg.club_player_id = rp.id
        JOIN tournaments t ON reg.tournament_id = t.id
        LEFT JOIN tournament_results tr ON tr.club_player_id = rp.id AND tr.tournament_id = t.id
        WHERE (u.id IS NULL OR (u.role = 'player' AND u.is_active = true))
            AND {filter}
        GROUP BY rp.id, rp.display_name, u.id, u.username, u.first_name, u.last_name,
                 u.email, u.phone, u.is_active, u.role, u.locale
        HAVING COUNT(DISTINCT reg.tournament_id) > 0
        "#,
        filter = tournament_filter
    );

    let mut stats_q = sqlx::query(&stats_sql)
        .bind(club_id)
        .bind(period_start)
        .bind(period_end);
    if tagged {
        stats_q = stats_q.bind(config_id);
    }
    let stat_rows = stats_q.fetch_all(pool).await?;

    // --- Query 2: per-result inputs for the points formula ---
    let results_sql = format!(
        r#"
        SELECT
            tr.club_player_id as club_player_id,
            tr.final_position as rank,
            t.buy_in_cents as buy_in_cents,
            f.field_size as field_size
        FROM tournament_results tr
        JOIN tournaments t ON t.id = tr.tournament_id
        JOIN (
            SELECT tournament_id, COUNT(*) as field_size
            FROM tournament_registrations GROUP BY tournament_id
        ) f ON f.tournament_id = tr.tournament_id
        WHERE tr.final_position > 0 AND {filter}
        "#,
        filter = tournament_filter
    );

    let mut results_q = sqlx::query(&results_sql)
        .bind(club_id)
        .bind(period_start)
        .bind(period_end);
    if tagged {
        results_q = results_q.bind(config_id);
    }
    let result_rows = results_q.fetch_all(pool).await?;

    // Per-player list of per-tournament point values.
    let mut points_by_player: HashMap<Uuid, Vec<u32>> = HashMap::new();
    for row in &result_rows {
        let club_player_id: Uuid = row.try_get("club_player_id")?;
        let rank: i32 = row.try_get("rank")?;
        let buy_in_cents: i32 = row.try_get("buy_in_cents")?;
        let field_size: i64 = row.try_get("field_size")?;
        let pts = event_points_with(
            formula,
            field_size as u32,
            rank.max(0) as u32,
            buy_in_cents as f64 / 100.0,
        );
        points_by_player
            .entry(club_player_id)
            .or_default()
            .push(pts);
    }

    // Best-N aggregation: sum a player's top N results (all if None).
    let sum_best = |mut vals: Vec<u32>| -> i64 {
        vals.sort_unstable_by(|a, b| b.cmp(a));
        let take = formula
            .count_best_n
            .map(|n| n as usize)
            .unwrap_or(vals.len());
        vals.iter().take(take).map(|&p| p as i64).sum()
    };

    let adjustments = crate::repos::leaderboard_adjustments::sum_by_player(pool, config_id).await?;

    // --- Merge into entries ---
    let mut entries: Vec<LeaderboardEntry> = Vec::with_capacity(stat_rows.len());
    for row in stat_rows {
        let club_player_id: Uuid = row.try_get("club_player_id")?;
        let total_tournaments = row.try_get::<i64, _>("total_tournaments")? as i32;
        let total_buy_ins = row.try_get::<i64, _>("total_buy_ins")? as i32;
        let total_winnings = row.try_get::<i64, _>("total_winnings")? as i32;
        let total_itm = row.try_get::<i64, _>("total_itm")? as i32;

        let base_points = points_by_player
            .remove(&club_player_id)
            .map(sum_best)
            .unwrap_or(0);
        let adjustment = adjustments.get(&club_player_id).copied().unwrap_or(0);
        let points = (base_points + adjustment) as f64;

        let itm_percentage = if total_tournaments > 0 {
            ((total_itm as f64 / total_tournaments as f64) * 100.0 * 100.0).round() / 100.0
        } else {
            0.0
        };
        let roi_percentage = if total_buy_ins > 0 {
            (((total_winnings - total_buy_ins) as f64 / total_buy_ins as f64) * 100.0 * 100.0)
                .round()
                / 100.0
        } else {
            0.0
        };
        let average_finish = (row.try_get::<f64, _>("average_finish")? * 100.0).round() / 100.0;

        entries.push(LeaderboardEntry {
            club_player_id,
            display_name: row.try_get("display_name")?,
            user_id: row.try_get("user_id")?,
            username: row.try_get("username")?,
            first_name: row.try_get("first_name")?,
            last_name: row.try_get("last_name")?,
            email: row.try_get("email")?,
            phone: row.try_get("phone")?,
            is_active: row.try_get("is_active")?,
            role: row.try_get("role")?,
            locale: row.try_get("locale")?,
            total_tournaments,
            total_buy_ins,
            total_winnings,
            net_profit: total_winnings - total_buy_ins,
            total_itm,
            itm_percentage,
            roi_percentage,
            average_finish,
            first_places: row.try_get::<i64, _>("first_places")? as i32,
            final_tables: row.try_get::<i64, _>("final_tables")? as i32,
            points,
        });
    }

    // Sort like the legacy leaderboard, then paginate in Rust.
    entries.sort_by(|a, b| {
        b.points
            .partial_cmp(&a.points)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.total_winnings.cmp(&a.total_winnings))
            .then(b.total_tournaments.cmp(&a.total_tournaments))
    });

    let total_count = entries.len() as i64;
    let offset_value = offset.unwrap_or(0).max(0) as usize;
    let limit_value = limit.unwrap_or(100).clamp(1, 500) as usize;
    let page = entries
        .into_iter()
        .skip(offset_value)
        .take(limit_value)
        .collect();

    Ok((page, total_count))
}
