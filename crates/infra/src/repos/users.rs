use sqlx::{PgExecutor, PgPool, Result};
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

pub async fn list(
    pool: &PgPool,
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

    let rows: Vec<UserRow> = query.build_query_as::<UserRow>().fetch_all(pool).await?;

    Ok(rows)
}

pub async fn count(pool: &PgPool, filter: UserFilter) -> Result<i64> {
    let mut query = sqlx::QueryBuilder::new("SELECT COUNT(*) as count FROM users WHERE 1=1");

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

    let result: (i64,) = query.build_query_as().fetch_one(pool).await?;
    Ok(result.0)
}

pub async fn get_by_id<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> Result<Option<UserRow>> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, email, username, first_name, last_name, phone, is_active, role, created_at, updated_at FROM users WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

pub async fn create<'e>(executor: impl PgExecutor<'e>, data: CreateUserData) -> Result<UserRow> {
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
    .fetch_one(executor)
    .await?;

    Ok(row)
}

pub async fn update<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    data: UpdateUserData,
) -> Result<Option<UserRow>> {
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
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

pub async fn deactivate<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> Result<Option<UserRow>> {
    let row = sqlx::query_as::<_, UserRow>(
        r#"
        UPDATE users
        SET is_active = false, updated_at = NOW()
        WHERE id = $1
        RETURNING id, email, username, first_name, last_name, phone, is_active, role, created_at, updated_at
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

pub async fn reactivate<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> Result<Option<UserRow>> {
    let row = sqlx::query_as::<_, UserRow>(
        r#"
        UPDATE users
        SET is_active = true, updated_at = NOW()
        WHERE id = $1
        RETURNING id, email, username, first_name, last_name, phone, is_active, role, created_at, updated_at
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}
