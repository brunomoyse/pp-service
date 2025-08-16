use sqlx::PgPool;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use api::app::build_router;
use api::gql::build_schema;
use api::services::spawn_clock_service;
use api::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();

    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    tracing::info!("Connected to Postgres");

    // Run database migrations automatically on startup (can be disabled with SKIP_MIGRATIONS=true)
    let skip_migrations = std::env::var("SKIP_MIGRATIONS")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);

    if skip_migrations {
        tracing::info!("Skipping database migrations (SKIP_MIGRATIONS=true)");
    } else {
        tracing::info!("Running database migrations...");
        sqlx::migrate!("../../migrations").run(&pool).await?;
        tracing::info!("Database migrations completed successfully");
    }

    let state = AppState::new(pool)?;

    // Build GraphQL schema from the gql module
    let schema = build_schema(state.clone());

    // Start the background clock service for auto-advancing tournament levels
    let _clock_handle = spawn_clock_service(state.clone());
    tracing::info!("Tournament clock service started");

    let app = build_router(state, schema);

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse()?;
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("Listening on {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
