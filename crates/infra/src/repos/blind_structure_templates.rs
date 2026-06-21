use serde_json::Value as JsonValue;
use sqlx::{PgExecutor, Result};
use uuid::Uuid;

use crate::models::BlindStructureTemplateRow;

#[derive(Debug, Clone)]
pub struct CreateBlindStructureTemplate {
    pub club_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub levels: JsonValue,
}

#[derive(Debug, Clone)]
pub struct UpdateBlindStructureTemplate {
    pub name: String,
    pub description: Option<String>,
    pub levels: JsonValue,
}

pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    data: CreateBlindStructureTemplate,
) -> Result<BlindStructureTemplateRow> {
    let row = sqlx::query_as::<_, BlindStructureTemplateRow>(
        r#"
        INSERT INTO blind_structure_templates (club_id, name, description, levels)
        VALUES ($1, $2, $3, $4)
        RETURNING id, club_id, name, description, levels, created_at, updated_at
        "#,
    )
    .bind(data.club_id)
    .bind(data.name)
    .bind(data.description)
    .bind(data.levels)
    .fetch_one(executor)
    .await?;

    Ok(row)
}

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> Result<Option<BlindStructureTemplateRow>> {
    let row = sqlx::query_as::<_, BlindStructureTemplateRow>(
        r#"
        SELECT id, club_id, name, description, levels, created_at, updated_at
        FROM blind_structure_templates
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

/// All blind structure templates owned by a club.
pub async fn list_by_club<'e>(
    executor: impl PgExecutor<'e>,
    club_id: Uuid,
) -> Result<Vec<BlindStructureTemplateRow>> {
    let rows = sqlx::query_as::<_, BlindStructureTemplateRow>(
        r#"
        SELECT id, club_id, name, description, levels, created_at, updated_at
        FROM blind_structure_templates
        WHERE club_id = $1
        ORDER BY name ASC
        "#,
    )
    .bind(club_id)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

pub async fn update<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    data: UpdateBlindStructureTemplate,
) -> Result<BlindStructureTemplateRow> {
    let row = sqlx::query_as::<_, BlindStructureTemplateRow>(
        r#"
        UPDATE blind_structure_templates
        SET name = $2, description = $3, levels = $4, updated_at = NOW()
        WHERE id = $1
        RETURNING id, club_id, name, description, levels, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(data.name)
    .bind(data.description)
    .bind(data.levels)
    .fetch_one(executor)
    .await?;

    Ok(row)
}

pub async fn delete<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM blind_structure_templates WHERE id = $1")
        .bind(id)
        .execute(executor)
        .await?;

    Ok(result.rows_affected() > 0)
}
