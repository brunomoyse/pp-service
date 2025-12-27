use sqlx::{PgPool, Result};
use uuid::Uuid;

use crate::{models::UserRow, pagination::LimitOffset};

#[derive(Debug, Clone)]
pub struct UserFilter {
    pub search: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct CreateUserData {
    pub email: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateUserData {
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub phone: Option<String>,
}

pub struct UserRepo {
    db: PgPool,
}

impl UserRepo {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn list(
        &self,
        filter: UserFilter,
        page: Option<LimitOffset>,
    ) -> Result<Vec<UserRow>> {
        let page = page.unwrap_or_default();

        let mut query = sqlx::QueryBuilder::new(
            "SELECT id, email, username, first_name, last_name, phone, is_active, role, created_at, updated_at FROM users WHERE 1=1"
        );

        if let Some(search) = &filter.search {
            let search_pattern = format!("%{}%", search.to_lowercase());
            query.push(" AND (");
            query.push("LOWER(username) LIKE ");
            query.push_bind(search_pattern.clone());
            query.push(" OR LOWER(first_name) LIKE ");
            query.push_bind(search_pattern.clone());
            query.push(" OR LOWER(last_name) LIKE ");
            query.push_bind(search_pattern);
            query.push(")");
        }

        if let Some(is_active) = filter.is_active {
            query.push(" AND is_active = ");
            query.push_bind(is_active);
        }

        query.push(" ORDER BY created_at DESC");
        query.push(" LIMIT ");
        query.push_bind(page.limit);
        query.push(" OFFSET ");
        query.push_bind(page.offset);

        let rows: Vec<UserRow> = query
            .build_query_as::<UserRow>()
            .fetch_all(&self.db)
            .await?;

        Ok(rows)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<UserRow>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, email, username, first_name, last_name, phone, is_active, role, created_at, updated_at FROM users WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    /// Create a new user (player)
    pub async fn create(&self, data: CreateUserData) -> Result<UserRow> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            INSERT INTO users (email, first_name, last_name, username, phone, role, is_active)
            VALUES ($1, $2, $3, $4, $5, 'player', true)
            RETURNING id, email, username, first_name, last_name, phone, is_active, role, created_at, updated_at
            "#,
        )
        .bind(&data.email)
        .bind(&data.first_name)
        .bind(&data.last_name)
        .bind(&data.username)
        .bind(&data.phone)
        .fetch_one(&self.db)
        .await?;

        Ok(row)
    }

    /// Update an existing user
    pub async fn update(&self, id: Uuid, data: UpdateUserData) -> Result<Option<UserRow>> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            UPDATE users
            SET email = COALESCE($2, email),
                first_name = COALESCE($3, first_name),
                last_name = COALESCE($4, last_name),
                username = COALESCE($5, username),
                phone = COALESCE($6, phone),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, email, username, first_name, last_name, phone, is_active, role, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&data.email)
        .bind(&data.first_name)
        .bind(&data.last_name)
        .bind(&data.username)
        .bind(&data.phone)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    /// Deactivate a user (soft delete)
    pub async fn deactivate(&self, id: Uuid) -> Result<Option<UserRow>> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            UPDATE users
            SET is_active = false, updated_at = NOW()
            WHERE id = $1
            RETURNING id, email, username, first_name, last_name, phone, is_active, role, created_at, updated_at
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    /// Reactivate a user
    pub async fn reactivate(&self, id: Uuid) -> Result<Option<UserRow>> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            UPDATE users
            SET is_active = true, updated_at = NOW()
            WHERE id = $1
            RETURNING id, email, username, first_name, last_name, phone, is_active, role, created_at, updated_at
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }
}
