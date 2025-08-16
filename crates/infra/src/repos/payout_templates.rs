use serde_json::Value as JsonValue;
use sqlx::{PgPool, Result};
use uuid::Uuid;

use crate::models::PayoutTemplateRow;

#[derive(Debug, Clone)]
pub struct CreatePayoutTemplate {
    pub name: String,
    pub description: Option<String>,
    pub min_players: i32,
    pub max_players: Option<i32>,
    pub payout_structure: JsonValue,
}

pub struct PayoutTemplateRepo {
    db: PgPool,
}

impl PayoutTemplateRepo {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn create(&self, data: CreatePayoutTemplate) -> Result<PayoutTemplateRow> {
        let row = sqlx::query_as::<_, PayoutTemplateRow>(
            r#"
            INSERT INTO payout_templates (name, description, min_players, max_players, payout_structure)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, name, description, min_players, max_players, payout_structure, created_at, updated_at
            "#
        )
        .bind(data.name)
        .bind(data.description)
        .bind(data.min_players)
        .bind(data.max_players)
        .bind(data.payout_structure)
        .fetch_one(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<PayoutTemplateRow>> {
        let row = sqlx::query_as::<_, PayoutTemplateRow>(
            r#"
            SELECT id, name, description, min_players, max_players, payout_structure, created_at, updated_at
            FROM payout_templates
            WHERE id = $1
            "#
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn list_all(&self) -> Result<Vec<PayoutTemplateRow>> {
        let rows = sqlx::query_as::<_, PayoutTemplateRow>(
            r#"
            SELECT id, name, description, min_players, max_players, payout_structure, created_at, updated_at
            FROM payout_templates
            ORDER BY name ASC
            "#
        )
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }

    pub async fn find_suitable_templates(
        &self,
        player_count: i32,
    ) -> Result<Vec<PayoutTemplateRow>> {
        let rows = sqlx::query_as::<_, PayoutTemplateRow>(
            r#"
            SELECT id, name, description, min_players, max_players, payout_structure, created_at, updated_at
            FROM payout_templates
            WHERE min_players <= $1 AND (max_players IS NULL OR max_players >= $1)
            ORDER BY min_players ASC, name ASC
            "#
        )
        .bind(player_count)
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }

    pub async fn update(&self, id: Uuid, data: CreatePayoutTemplate) -> Result<PayoutTemplateRow> {
        let row = sqlx::query_as::<_, PayoutTemplateRow>(
            r#"
            UPDATE payout_templates 
            SET name = $2, description = $3, min_players = $4, max_players = $5, payout_structure = $6, updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, description, min_players, max_players, payout_structure, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(data.name)
        .bind(data.description)
        .bind(data.min_players)
        .bind(data.max_players)
        .bind(data.payout_structure)
        .fetch_one(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM payout_templates WHERE id = $1")
            .bind(id)
            .execute(&self.db)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
