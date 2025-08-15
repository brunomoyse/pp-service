use sqlx::{PgPool, Result};
use uuid::Uuid;

use crate::{models::UserRow, pagination::LimitOffset};

#[derive(Debug, Clone)]
pub struct UserFilter {
    pub search: Option<String>,
    pub is_active: Option<bool>,
}

pub struct UserRepo {
    db: PgPool,
}

impl UserRepo {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn list(&self, filter: UserFilter, page: Option<LimitOffset>) -> Result<Vec<UserRow>> {
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
}