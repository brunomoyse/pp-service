use serde_json::Value as JsonValue;
use sqlx::{PgExecutor, Result};
use uuid::Uuid;

use crate::models::BlindStructureTemplateRow;

#[derive(Debug, Clone)]
pub struct CreateBlindStructureTemplate {
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
        INSERT INTO blind_structure_templates (name, description, levels)
        VALUES ($1, $2, $3)
        RETURNING id, name, description, levels, created_at, updated_at
        "#,
    )
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
        SELECT id, name, description, levels, created_at, updated_at
        FROM blind_structure_templates
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

pub async fn list<'e>(executor: impl PgExecutor<'e>) -> Result<Vec<BlindStructureTemplateRow>> {
    let rows = sqlx::query_as::<_, BlindStructureTemplateRow>(
        r#"
        SELECT id, name, description, levels, created_at, updated_at
        FROM blind_structure_templates
        ORDER BY name ASC
        "#,
    )
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

pub async fn update<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    data: CreateBlindStructureTemplate,
) -> Result<BlindStructureTemplateRow> {
    let row = sqlx::query_as::<_, BlindStructureTemplateRow>(
        r#"
        UPDATE blind_structure_templates
        SET name = $2, description = $3, levels = $4, updated_at = NOW()
        WHERE id = $1
        RETURNING id, name, description, levels, created_at, updated_at
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
