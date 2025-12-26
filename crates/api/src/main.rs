use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use api::app::build_router;
use api::gql::build_schema;
use api::services::{spawn_clock_service, spawn_notification_service};
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

    // Configure connection pool with appropriate limits
    let max_connections: u32 = std::env::var("DATABASE_MAX_CONNECTIONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(std::time::Duration::from_secs(3))
        .idle_timeout(Some(std::time::Duration::from_secs(600))) // 10 minutes
        .max_lifetime(Some(std::time::Duration::from_secs(1800))) // 30 minutes
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    tracing::info!(
        "Connected to Postgres with max {} connections",
        max_connections
    );

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

    // Start the background notification service for "tournament starting soon" alerts
    let _notification_handle = spawn_notification_service(state.clone());
    tracing::info!("Notification service started");

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
