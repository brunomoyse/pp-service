use std::time::Duration;

use axum::{
    extract::State,
    response::Html,
    routing::get,
    Router,
};
use async_graphql::{http::GraphiQLSource, ObjectType, Schema, SubscriptionType};
use async_graphql_axum::{GraphQL, GraphQLSubscription};
use axum::routing::{post_service};
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};

use crate::error::AppError;
use crate::state::AppState;

/// Build the Axum router with health endpoint and GraphQL
/// Generic over your schema roots so you can keep `QueryRoot` in `main.rs` (or elsewhere).
pub fn build_router<Q, M, S>(state: AppState, schema: Schema<Q, M, S>) -> Router
where
    Q: ObjectType + Send + Sync + 'static,
    M: ObjectType + Send + Sync + 'static,
    S: SubscriptionType + Send + Sync + 'static,
{
    let gql_http = GraphQL::new(schema.clone());
    let gql_ws   = GraphQLSubscription::new(schema);

    Router::new()
        // Simple liveness check; also proves DB connectivity.
        .route("/health", get(health))
        // graphql post & subscription
        .route(
            "/graphql",
            post_service(gql_http).get_service(gql_ws),
        )
        // App state (PgPool, broadcasters, etc.)
        .with_state(state)
        // Useful default middlewares
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(CorsLayer::permissive()) // tighten later
}

/// Liveness + quick DB probe.
async fn health(State(state): State<AppState>) -> Result<&'static str, AppError> {
    // Inexpensive round-trip; replace by `SELECT 1` if you prefer.
    let _one: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&state.db).await?;
    Ok("ok")
}