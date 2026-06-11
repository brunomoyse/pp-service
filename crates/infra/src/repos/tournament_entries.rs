use sqlx::{PgExecutor, Result};
use uuid::Uuid;

use crate::models::TournamentEntryRow;

const COLS: &str = "id, tournament_id, user_id, club_player_id, entry_type, amount_cents, chips_received, recorded_by, notes, payment_method, created_at, updated_at";

#[derive(Debug, Clone, Default)]
pub struct CreateTournamentEntry {
    pub tournament_id: Uuid,
    /// App user, when the player has an account. The link trigger stamps
    /// whichever of user_id / club_player_id is missing.
    pub user_id: Option<Uuid>,
    pub club_player_id: Option<Uuid>,
    pub entry_type: String,
    pub amount_cents: i32,
    pub chips_received: Option<i32>,
    pub recorded_by: Option<Uuid>,
    pub notes: Option<String>,
    /// How the player paid: cash | card | bank_transfer | voucher | comp | other.
    /// An empty value (e.g. from `Default`) is coerced to "cash" in `create`.
    pub payment_method: String,
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
    pub total_rake_cents: i64,
    pub total_chips: i64,
    pub players_remaining: i64,
}

pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    data: CreateTournamentEntry,
) -> Result<TournamentEntryRow> {
    let payment_method = if data.payment_method.is_empty() {
        "cash".to_string()
    } else {
        data.payment_method
    };
    let row = sqlx::query_as::<_, TournamentEntryRow>(&format!(
        "INSERT INTO tournament_entries \
            (tournament_id, user_id, club_player_id, entry_type, amount_cents, chips_received, recorded_by, notes, payment_method) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING {COLS}"
    ))
    .bind(data.tournament_id)
    .bind(data.user_id)
    .bind(data.club_player_id)
    .bind(data.entry_type)
    .bind(data.amount_cents)
    .bind(data.chips_received)
    .bind(data.recorded_by)
    .bind(data.notes)
    .bind(payment_method)
    .fetch_one(executor)
    .await?;

    Ok(row)
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> Result<Option<TournamentEntryRow>> {
    let row = sqlx::query_as::<_, TournamentEntryRow>(&format!(
        "SELECT {COLS} FROM tournament_entries WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

pub async fn list_by_tournament<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> Result<Vec<TournamentEntryRow>> {
    let rows = sqlx::query_as::<_, TournamentEntryRow>(&format!(
        "SELECT {COLS} FROM tournament_entries WHERE tournament_id = $1 ORDER BY created_at ASC"
    ))
    .bind(tournament_id)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

pub async fn list_by_tournament_and_user<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<TournamentEntryRow>> {
    let rows = sqlx::query_as::<_, TournamentEntryRow>(&format!(
        "SELECT {COLS} FROM tournament_entries WHERE tournament_id = $1 AND user_id = $2 ORDER BY created_at ASC"
    ))
    .bind(tournament_id)
    .bind(user_id)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

pub async fn get_stats<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> Result<TournamentEntryStats> {
    let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)>(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE e.entry_type IN ('initial', 'rebuy', 're_entry')) as total_entries,
            COALESCE(SUM(e.amount_cents), 0) as total_amount_cents,
            COUNT(DISTINCT e.club_player_id) as unique_players,
            COUNT(*) FILTER (WHERE e.entry_type = 'initial') as initial_count,
            COUNT(*) FILTER (WHERE e.entry_type = 'rebuy') as rebuy_count,
            COUNT(*) FILTER (WHERE e.entry_type = 're_entry') as re_entry_count,
            COUNT(*) FILTER (WHERE e.entry_type = 'addon') as addon_count,
            COALESCE(
                COUNT(*) FILTER (WHERE e.entry_type IN ('initial', 're_entry'))
                * (SELECT rake_cents FROM tournaments WHERE id = $1),
                0
            ) as total_rake_cents,
            COALESCE(SUM(e.chips_received), 0)
                + (SELECT COALESCE(COUNT(*) FILTER (WHERE r.early_bird_bonus_awarded), 0)
                       * (SELECT COALESCE(early_bird_bonus_chips, 0) FROM tournaments WHERE id = $1)
                       + COALESCE(COUNT(*) FILTER (WHERE r.level_two_bonus_awarded), 0)
                       * (SELECT COALESCE(level_two_bonus_chips, 0) FROM tournaments WHERE id = $1)
                   FROM tournament_registrations r
                   WHERE r.tournament_id = $1) as total_chips,
            (SELECT COUNT(*) FROM tournament_registrations r
             WHERE r.tournament_id = $1
               AND r.status IN ('registered', 'checked_in', 'seated')) as players_remaining
        FROM tournament_entries e
        WHERE e.tournament_id = $1
        "#,
    )
    .bind(tournament_id)
    .fetch_one(executor)
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
        total_rake_cents: row.7,
        total_chips: row.8,
        players_remaining: row.9,
    })
}

/// One row of the end-of-night cash report: money taken in for a given
/// (payment method, entry type) pair. The resolver pivots these into a
/// method-by-type matrix and per-method totals.
#[derive(Debug, Clone)]
pub struct CashReportLine {
    pub payment_method: String,
    pub entry_type: String,
    pub amount_cents: i64,
    pub count: i64,
}

pub async fn get_cash_report<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> Result<Vec<CashReportLine>> {
    let rows = sqlx::query_as::<_, (String, String, i64, i64)>(
        r#"
        SELECT payment_method,
               entry_type,
               COALESCE(SUM(amount_cents), 0) AS amount_cents,
               COUNT(*) AS cnt
        FROM tournament_entries
        WHERE tournament_id = $1
        GROUP BY payment_method, entry_type
        ORDER BY payment_method, entry_type
        "#,
    )
    .bind(tournament_id)
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(payment_method, entry_type, amount_cents, count)| CashReportLine {
                payment_method,
                entry_type,
                amount_cents,
                count,
            },
        )
        .collect())
}

pub async fn delete<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM tournament_entries WHERE id = $1")
        .bind(id)
        .execute(executor)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn get_total_prize_pool<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
) -> Result<i64> {
    let result: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM tournament_entries WHERE tournament_id = $1",
    )
    .bind(tournament_id)
    .fetch_one(executor)
    .await?;
    Ok(result.0)
}

pub async fn apply_early_bird_bonus<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    user_id: Uuid,
    bonus_chips: i32,
) -> Result<Option<TournamentEntryRow>> {
    let row = sqlx::query_as::<_, TournamentEntryRow>(&format!(
        "UPDATE tournament_entries \
         SET chips_received = COALESCE(chips_received, 0) + $3, updated_at = NOW() \
         WHERE tournament_id = $1 AND user_id = $2 AND entry_type = 'initial' RETURNING {COLS}"
    ))
    .bind(tournament_id)
    .bind(user_id)
    .bind(bonus_chips)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

/// Grant the level-2 early-bird bonus to the given roster players. For each one
/// that is still `seated` and not yet awarded, inserts a chip-only `bonus` entry
/// (excluded from the prize pool) and flips `level_two_bonus_awarded`. Idempotent:
/// re-running awards nothing further. Returns the number of players awarded.
pub async fn grant_level_two_bonus<'e>(
    executor: impl PgExecutor<'e>,
    tournament_id: Uuid,
    club_player_ids: &[Uuid],
    bonus_chips: i32,
) -> Result<i64> {
    if club_player_ids.is_empty() {
        return Ok(0);
    }

    let count = sqlx::query_scalar::<_, i64>(
        r#"
        WITH eligible AS (
            SELECT reg.club_player_id
            FROM tournament_registrations reg
            WHERE reg.tournament_id = $1
              AND reg.club_player_id = ANY($2::uuid[])
              AND reg.status = 'seated'
              AND reg.level_two_bonus_awarded = false
        ),
        ins AS (
            INSERT INTO tournament_entries
                (tournament_id, club_player_id, user_id, entry_type,
                 amount_cents, chips_received, notes)
            SELECT $1, e.club_player_id, NULL, 'bonus', 0, $3,
                   'Level-2 early-bird bonus'
            FROM eligible e
            RETURNING club_player_id
        ),
        upd AS (
            UPDATE tournament_registrations reg
            SET level_two_bonus_awarded = true, updated_at = NOW()
            FROM ins
            WHERE reg.tournament_id = $1
              AND reg.club_player_id = ins.club_player_id
            RETURNING reg.club_player_id
        )
        SELECT COUNT(*) FROM upd
        "#,
    )
    .bind(tournament_id)
    .bind(club_player_ids)
    .bind(bonus_chips)
    .fetch_one(executor)
    .await?;

    Ok(count)
}

pub async fn apply_early_bird_bonus_bulk<'e>(
    executor: impl PgExecutor<'e>,
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
    .execute(executor)
    .await?;

    Ok(result.rows_affected())
}
