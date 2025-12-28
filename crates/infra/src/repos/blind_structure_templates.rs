use sqlx::{PgPool, Result};
use uuid::Uuid;

use crate::models::BlindStructureTemplateRow;

pub struct BlindStructureTemplateRepo {
    db: PgPool,
}

impl BlindStructureTemplateRepo {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<BlindStructureTemplateRow>> {
        let row = sqlx::query_as::<_, BlindStructureTemplateRow>(
            r#"
            SELECT id, name, description, levels, created_at
            FROM blind_structure_templates
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn list_all(&self) -> Result<Vec<BlindStructureTemplateRow>> {
        let rows = sqlx::query_as::<_, BlindStructureTemplateRow>(
            r#"
            SELECT id, name, description, levels, created_at
            FROM blind_structure_templates
            ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }
}
