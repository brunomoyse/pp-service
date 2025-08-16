use std::time::Duration;

use async_graphql::{ObjectType, Schema, SubscriptionType};
use async_graphql_axum::GraphQLSubscription;
use axum::{
    extract::{Request, State},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};

use crate::auth::Claims;
use crate::error::AppError;
use crate::middleware::jwt::jwt_middleware;
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
    let gql_ws = GraphQLSubscription::new(schema.clone());

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
        // GraphQL endpoint with custom handler that includes JWT claims in context
        .route(
            "/graphql",
            post({
                let schema_clone = schema.clone();
                move |state, req| graphql_handler(state, req, schema_clone)
            })
            .get_service(gql_ws),
        )
        // App state (PgPool, broadcasters, etc.)
        .with_state(state.clone())
        // JWT middleware for authentication
        .layer(middleware::from_fn_with_state(state, jwt_middleware))
        // Useful default middlewares
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(CorsLayer::permissive()) // tighten later
}

/// Custom GraphQL handler that extracts JWT claims from request extensions
/// and adds them to the GraphQL context
async fn graphql_handler<Q, M, S>(
    State(state): State<AppState>,
    req: Request,
    schema: Schema<Q, M, S>,
) -> Result<Response, AppError>
where
    Q: ObjectType + Send + Sync + 'static,
    M: ObjectType + Send + Sync + 'static,
    S: SubscriptionType + Send + Sync + 'static,
{
    // Extract claims from request extensions (set by JWT middleware)
    let claims = req.extensions().get::<Claims>().cloned();

    // Extract the GraphQL request from the HTTP request
    let (_parts, body) = req.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to read request body: {}", e)))?;

    let gql_request: async_graphql::Request = serde_json::from_slice(&body_bytes)
        .map_err(|e| AppError::BadRequest(format!("Invalid GraphQL request: {}", e)))?;

    // Add the AppState and optionally claims to the GraphQL context
    let mut gql_request = gql_request.data(state);
    if let Some(claims) = claims {
        gql_request = gql_request.data(claims);
    }

    // Execute the GraphQL request
    let gql_response = schema.execute(gql_request).await;

    Ok(Json(gql_response).into_response())
}

/// Liveness + quick DB probe.
async fn health(State(state): State<AppState>) -> Result<&'static str, AppError> {
    // Inexpensive round-trip; replace by `SELECT 1` if you prefer.
    let _one: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&state.db).await?;
    Ok("ok")
}
