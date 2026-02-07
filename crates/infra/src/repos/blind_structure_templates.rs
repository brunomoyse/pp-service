use sqlx::{PgExecutor, Result};
use uuid::Uuid;

use crate::models::BlindStructureTemplateRow;

pub async fn get_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
) -> Result<Option<BlindStructureTemplateRow>> {
    let row = sqlx::query_as::<_, BlindStructureTemplateRow>(
        r#"
        SELECT id, name, description, levels, created_at
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
        SELECT id, name, description, levels, created_at
        FROM blind_structure_templates
        ORDER BY name ASC
        "#,
    )
    .fetch_all(executor)
    .await?;

    Ok(rows)
}
