use sqlx::{PgPool, Result};
use uuid::Uuid;

use crate::models::TournamentRegistrationRow;

#[derive(Debug, Clone)]
pub struct CreateTournamentRegistration {
    pub tournament_id: Uuid,
    pub user_id: Uuid,
    pub notes: Option<String>,
}

pub struct TournamentRegistrationRepo {
    db: PgPool,
}

impl TournamentRegistrationRepo {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn create(
        &self,
        data: CreateTournamentRegistration,
    ) -> Result<TournamentRegistrationRow> {
        let row = sqlx::query_as::<_, TournamentRegistrationRow>(
            r#"
            INSERT INTO tournament_registrations (tournament_id, user_id, notes)
            VALUES ($1, $2, $3)
            RETURNING id, tournament_id, user_id, registration_time, status, notes, created_at, updated_at
            "#
        )
        .bind(data.tournament_id)
        .bind(data.user_id)
        .bind(data.notes)
        .fetch_one(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<TournamentRegistrationRow>> {
        let row = sqlx::query_as::<_, TournamentRegistrationRow>(
            r#"
            SELECT id, tournament_id, user_id, registration_time, status, notes, created_at, updated_at
            FROM tournament_registrations
            WHERE id = $1
            "#
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn get_by_tournament_and_user(
        &self,
        tournament_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<TournamentRegistrationRow>> {
        let row = sqlx::query_as::<_, TournamentRegistrationRow>(
            r#"
            SELECT id, tournament_id, user_id, registration_time, status, notes, created_at, updated_at
            FROM tournament_registrations
            WHERE tournament_id = $1 AND user_id = $2
            "#
        )
        .bind(tournament_id)
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    pub async fn get_by_tournament(
        &self,
        tournament_id: Uuid,
    ) -> Result<Vec<TournamentRegistrationRow>> {
        let rows = sqlx::query_as::<_, TournamentRegistrationRow>(
            r#"
            SELECT id, tournament_id, user_id, registration_time, status, notes, created_at, updated_at
            FROM tournament_registrations
            WHERE tournament_id = $1
            ORDER BY registration_time ASC
            "#
        )
        .bind(tournament_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }

    pub async fn get_user_current_registrations(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<TournamentRegistrationRow>> {
        let rows = sqlx::query_as::<_, TournamentRegistrationRow>(
            r#"
            SELECT tr.id, tr.tournament_id, tr.user_id, tr.registration_time, tr.status, tr.notes, tr.created_at, tr.updated_at
            FROM tournament_registrations tr
            JOIN tournaments t ON tr.tournament_id = t.id
            WHERE tr.user_id = $1 AND (t.end_time IS NULL OR t.end_time > NOW())
            ORDER BY tr.created_at DESC
            "#
        )
        .bind(user_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }
}
