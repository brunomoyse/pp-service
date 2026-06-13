// The merged GraphQL schema generates deeply-nested async future types; laying
// them out overflows the default 128 recursion limit.
#![recursion_limit = "512"]

use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tokio::sync::watch;

use api::app::build_router;
use api::gql::build_schema;
use api::services::{
    data_retention_service, spawn_clock_service, spawn_data_retention_service,
    spawn_drink_expiry_service, spawn_notification_service, supervise,
};
use api::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    api::observability::init_tracing();

    dotenvy::dotenv().ok();

    // Configure connection pool with appropriate limits
    let max_connections: u32 = std::env::var("DATABASE_MAX_CONNECTIONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections(5) // Pre-warm pool with 5 connections
        .acquire_timeout(std::time::Duration::from_secs(10))
        .idle_timeout(Some(std::time::Duration::from_secs(300))) // 5 minutes (reduced from 10)
        .max_lifetime(Some(std::time::Duration::from_secs(1800))) // 30 minutes
        .test_before_acquire(true) // Verify connection is alive before using
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

    // Small delay to let the connection pool warm up before starting background services
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Coordinated shutdown: flipping this watch stops the supervised services.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Background services run under a supervisor that restarts them on panic and
    // stops them on shutdown. Previously their JoinHandles were dropped, so a
    // panic silently killed e.g. clock auto-advance with no recovery and /health
    // still reported OK.
    let _clock = supervise("clock_service", shutdown_rx.clone(), {
        let state = state.clone();
        move || spawn_clock_service(state.clone())
    });
    tracing::info!("Tournament clock service started");

    let _notification = supervise("notification_service", shutdown_rx.clone(), {
        let state = state.clone();
        move || spawn_notification_service(state.clone())
    });
    tracing::info!("Notification service started");

    let _drink_expiry = supervise("drink_expiry_service", shutdown_rx.clone(), {
        let state = state.clone();
        move || spawn_drink_expiry_service(state.clone())
    });
    tracing::info!("Drink credit expiry service started");

    // GDPR data-retention sweep — destructive (anonymizes dormant accounts), so
    // it only runs when explicitly enabled via ENABLE_DATA_RETENTION.
    let _data_retention = if data_retention_service::is_enabled() {
        let handle = supervise("data_retention_service", shutdown_rx.clone(), {
            let state = state.clone();
            move || spawn_data_retention_service(state.clone())
        });
        tracing::info!("Data-retention service started");
        Some(handle)
    } else {
        tracing::info!(
            "Data-retention service disabled (set ENABLE_DATA_RETENTION=true to enable)"
        );
        None
    };

    // Cross-instance real-time bus: the notifier broadcasts locally-published
    // events over Postgres NOTIFY; the listener fans events from other instances
    // into the local subscription channels. Both have internal error/reconnect
    // handling. Must be spawned before any request can publish an event.
    let _realtime_notifier = api::gql::realtime::spawn_realtime_notifier(state.db.clone());
    let _realtime_listener = api::gql::realtime::spawn_realtime_listener(state.db.clone());
    tracing::info!("Realtime bus (LISTEN/NOTIFY) started");

    let app = build_router(state, schema);

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse()?;
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("Listening on {}", addr);

    // Drain in-flight requests on SIGTERM/Ctrl-C instead of dropping them.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    // Server has stopped accepting; tell background services to stop and give
    // them a brief moment to wind down their current tick.
    tracing::info!("Server stopped; shutting down background services");
    let _ = shutdown_tx.send(true);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    Ok(())
}

/// Resolves when the process receives Ctrl-C or (on Unix) SIGTERM — the signal
/// orchestrators send on rollout/scale-down.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
