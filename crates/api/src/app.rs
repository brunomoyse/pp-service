use std::time::Duration;

use axum::{
    extract::State,
    routing::{get, post},
    Router,
};
use async_graphql::{ObjectType, Schema, SubscriptionType};
use async_graphql_axum::{GraphQL, GraphQLSubscription};
use axum::routing::{post_service};
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};

use crate::error::AppError;
use crate::routes::{auth, oauth_server, unified_auth};
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
        // Unified authentication choice
        .route("/auth/choose", get(unified_auth::auth_choice))
        // External OAuth authentication routes
        .route("/auth/{provider}/authorize", get(auth::authorize))
        .route("/auth/{provider}/callback", get(auth::callback))
        // Custom OAuth server routes
        .route("/oauth/authorize", get(oauth_server::authorize))
        .route("/oauth/login", post(oauth_server::login))
        .route("/oauth/token", post(oauth_server::token))
        .route("/oauth/register", get(oauth_server::register_form))
        .route("/oauth/register", post(oauth_server::register))
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