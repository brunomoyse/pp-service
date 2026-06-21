use serde_json::Value as JsonValue;
use sqlx::{PgExecutor, Result};
use uuid::Uuid;

use crate::models::PayoutTemplateRow;

#[derive(Debug, Clone)]
pub struct CreatePayoutTemplate {
    pub club_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub min_players: i32,
    pub max_players: Option<i32>,
    pub payout_structure: JsonValue,
}

#[derive(Debug, Clone)]
pub struct UpdatePayoutTemplate {
    pub name: String,
    pub description: Option<String>,
    pub min_players: i32,
    pub max_players: Option<i32>,
    pub payout_structure: JsonValue,
}

pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    data: CreatePayoutTemplate,
) -> Result<PayoutTemplateRow> {
    let row = sqlx::query_as::<_, PayoutTemplateRow>(
        r#"
        INSERT INTO payout_templates (club_id, name, description, min_players, max_players, payout_structure)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, club_id, name, description, min_players, max_players, payout_structure, created_at, updated_at
        "#
    )
    .bind(data.club_id)
    .bind(data.name)
    .bind(data.description)
    .bind(data.min_players)
    .bind(data.max_players)
    .bind(data.payout_structure)
    .fetch_one(executor)
    .await?;

    Ok(row)
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> Result<Option<PayoutTemplateRow>> {
    let row = sqlx::query_as::<_, PayoutTemplateRow>(
        r#"
        SELECT id, club_id, name, description, min_players, max_players, payout_structure, created_at, updated_at
        FROM payout_templates
        WHERE id = $1
        "#
    )
    .bind(id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

/// All payout templates owned by a club.
pub async fn list_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
) -> Result<Vec<PayoutTemplateRow>> {
    let rows = sqlx::query_as::<_, PayoutTemplateRow>(
        r#"
        SELECT id, club_id, name, description, min_players, max_players, payout_structure, created_at, updated_at
        FROM payout_templates
        WHERE club_id = $1
        ORDER BY min_players ASC, name ASC
        "#
    )
    .bind(club_id)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

/// Club-owned payout templates that fit a given player count.
pub async fn find_suitable_templates<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
    player_count: i32,
) -> Result<Vec<PayoutTemplateRow>> {
    let rows = sqlx::query_as::<_, PayoutTemplateRow>(
        r#"
        SELECT id, club_id, name, description, min_players, max_players, payout_structure, created_at, updated_at
        FROM payout_templates
        WHERE club_id = $1 AND min_players <= $2 AND (max_players IS NULL OR max_players >= $2)
        ORDER BY min_players ASC, name ASC
        "#
    )
    .bind(club_id)
    .bind(player_count)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

pub async fn update<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    data: UpdatePayoutTemplate,
) -> Result<PayoutTemplateRow> {
    let row = sqlx::query_as::<_, PayoutTemplateRow>(
        r#"
        UPDATE payout_templates
        SET name = $2, description = $3, min_players = $4, max_players = $5, payout_structure = $6, updated_at = NOW()
        WHERE id = $1
        RETURNING id, club_id, name, description, min_players, max_players, payout_structure, created_at, updated_at
        "#
    )
    .bind(id)
    .bind(data.name)
    .bind(data.description)
    .bind(data.min_players)
    .bind(data.max_players)
    .bind(data.payout_structure)
    .fetch_one(executor)
    .await?;

    Ok(row)
}

pub async fn delete<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM payout_templates WHERE id = $1")
        .bind(id)
        .execute(executor)
        .await?;

    Ok(result.rows_affected() > 0)
}
