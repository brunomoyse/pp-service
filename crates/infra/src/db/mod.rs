use sqlx::PgPool;

pub type Db = PgPool;

pub async fn ping(pool: &Db) -> Result<(), sqlx::Error> {
    let _: i32 = sqlx::query_scalar("SELECT 1").fetch_one(pool).await?;
    Ok(())
}