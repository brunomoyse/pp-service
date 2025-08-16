use serde_json::Value as JsonValue;
use sqlx::{PgPool, Result};
use uuid::Uuid;

use crate::models::PlayerDealRow;

#[derive(Debug, Clone)]
pub struct CreatePlayerDeal {
    pub tournament_id: Uuid,
    pub deal_type: String,
    pub affected_positions: Vec<i32>,
    pub custom_payouts: Option<JsonValue>,
    pub total_amount_cents: i32,
    pub notes: Option<String>,
    pub created_by: Uuid,
}

pub struct PlayerDealRepo {
    db: PgPool,
}

impl PlayerDealRepo {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn create(&self, data: CreatePlayerDeal) -> Result<PlayerDealRow> {
        let row = sqlx::query_as::<_, PlayerDealRow>(
            r#"
            INSERT INTO player_deals (tournament_id, deal_type, affected_positions, custom_payouts, total_amount_cents, notes, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, tournament_id, deal_type, affected_positions, custom_payouts, total_amount_cents, notes, created_by, created_at, updated_at
            "#
        )
        .bind(data.tournament_id)
        .bind(data.deal_type)
        .bind(&data.affected_positions)
        .bind(data.custom_payouts)
        .bind(data.total_amount_cents)
        .bind(data.notes)
        .bind(data.created_by)
        .fetch_one(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<PlayerDealRow>> {
        let row = sqlx::query_as::<_, PlayerDealRow>(
            r#"
            SELECT id, tournament_id, deal_type, affected_positions, custom_payouts, total_amount_cents, notes, created_by, created_at, updated_at
            FROM player_deals
            WHERE id = $1
            "#
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn get_by_tournament(&self, tournament_id: Uuid) -> Result<Option<PlayerDealRow>> {
        let row = sqlx::query_as::<_, PlayerDealRow>(
            r#"
            SELECT id, tournament_id, deal_type, affected_positions, custom_payouts, total_amount_cents, notes, created_by, created_at, updated_at
            FROM player_deals
            WHERE tournament_id = $1
            "#
        )
        .bind(tournament_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn update(&self, id: Uuid, data: CreatePlayerDeal) -> Result<PlayerDealRow> {
        let row = sqlx::query_as::<_, PlayerDealRow>(
            r#"
            UPDATE player_deals 
            SET tournament_id = $2, deal_type = $3, affected_positions = $4, custom_payouts = $5, total_amount_cents = $6, notes = $7, created_by = $8, updated_at = NOW()
            WHERE id = $1
            RETURNING id, tournament_id, deal_type, affected_positions, custom_payouts, total_amount_cents, notes, created_by, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(data.tournament_id)
        .bind(data.deal_type)
        .bind(&data.affected_positions)
        .bind(data.custom_payouts)
        .bind(data.total_amount_cents)
        .bind(data.notes)
        .bind(data.created_by)
        .fetch_one(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM player_deals WHERE id = $1")
            .bind(id)
            .execute(&self.db)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
