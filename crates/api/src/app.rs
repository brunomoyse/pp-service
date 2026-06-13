use std::sync::Arc;
use std::time::Duration;

use async_graphql::{ObjectType, Schema, SubscriptionType};
use async_graphql_axum::{GraphQLProtocol, GraphQLWebSocket};
use axum::{
    extract::{Request, State, WebSocketUpgrade},
    http::{
        header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE},
        Method, StatusCode,
    },
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_governor::GovernorLayer;
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};

use crate::auth::Claims;
use crate::error::AppError;
use crate::middleware::jwt::jwt_middleware;
use crate::observability::{correlation_id, render_metrics, track_metrics};
use crate::routes::{auth, oauth_server, token, unified_auth};
use crate::state::AppState;

/// Build the Axum router with health endpoint and GraphQL
/// Generic over your schema roots so you can keep `QueryRoot` in `main.rs` (or elsewhere).
pub fn build_router<Q, M, S>(state: AppState, schema: Schema<Q, M, S>) -> Router
where
    Q: ObjectType + Send + Sync + 'static,
    M: ObjectType + Send + Sync + 'static,
    S: SubscriptionType + Send + Sync + 'static,
{
    // Rate limiting: 10 requests per minute per IP on auth endpoints
    let governor_conf = GovernorConfigBuilder::default()
        .per_second(6) // 1 token every 6 seconds = ~10/min
        .burst_size(10)
        .finish()
        .unwrap();

    // Rate-limited auth routes (login + register POST)
    let rate_limited_routes = Router::new()
        .route("/oauth/login", post(oauth_server::login))
        .route("/oauth/register", post(oauth_server::register))
        .layer(GovernorLayer::new(Arc::new(governor_conf)));

    // Coarse abuse-protection on the GraphQL endpoint. Generous limits: real-time
    // load rides the WebSocket, so a client's HTTP request rate is low (login,
    // occasional queries/mutations). Uses the forwarded client IP (X-Forwarded-For)
    // so a whole venue behind one NAT isn't throttled as a single client and the
    // limit isn't collapsed to the reverse proxy's IP. ASSUMES a trusted proxy
    // (Caddy) overwrites X-Forwarded-For; if the service is ever exposed directly,
    // switch back to the peer-IP extractor to prevent header spoofing.
    let graphql_governor = GovernorConfigBuilder::default()
        .per_millisecond(100) // ~10 req/s sustained per client IP
        .burst_size(120)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .unwrap();
    let graphql_governor = Arc::new(graphql_governor);

    Router::new()
        // Simple liveness check; also proves DB connectivity.
        .route("/health", get(health))
        // Readiness probe for orchestrators (distinct from liveness): only OK
        // once the DB is reachable, so traffic isn't routed before we can serve.
        .route("/ready", get(ready))
        // Prometheus metrics. Exposed app-wide here; restrict to the internal
        // network / scraper at the reverse proxy in production.
        .route("/metrics", get(metrics))
        // Unified authentication choice
        .route("/auth/choose", get(unified_auth::auth_choice))
        // External OAuth authentication routes
        .route("/auth/{provider}/authorize", get(auth::authorize))
        .route("/auth/{provider}/callback", get(auth::callback))
        // Custom OAuth server routes (non-rate-limited)
        .route("/oauth/authorize", get(oauth_server::authorize))
        .route("/oauth/token", post(oauth_server::token))
        .route("/oauth/register", get(oauth_server::register_form))
        // Rate-limited auth routes
        .merge(rate_limited_routes)
        // Refresh token and logout endpoints (no JWT auth required — uses cookie)
        .route("/auth/refresh", post(token::refresh_handler))
        .route("/auth/logout", post(token::logout_handler))
        // GraphQL endpoint with custom handler that includes JWT claims in context
        .route(
            "/graphql",
            post({
                let schema_clone = schema.clone();
                move |state, req| graphql_handler(state, req, schema_clone)
            })
            .get({
                let schema_clone = schema.clone();
                move |state, protocol, upgrade| {
                    graphql_ws_handler(state, protocol, upgrade, schema_clone)
                }
            })
            .layer(GovernorLayer::new(graphql_governor)),
        )
        // App state (PgPool, broadcasters, etc.)
        .with_state(state.clone())
        // JWT middleware for authentication
        .layer(middleware::from_fn_with_state(state, jwt_middleware))
        // Useful default middlewares
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer({
            let allowed_origins = std::env::var("ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:3000,http://localhost:3001".to_string());

            let origins: Vec<HeaderValue> = allowed_origins
                .split(',')
                .filter_map(|o| o.trim().parse().ok())
                .collect();

            CorsLayer::new()
                .allow_origin(origins)
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([CONTENT_TYPE, AUTHORIZATION])
                .allow_credentials(true)
        })
        // Count requests/responses for /metrics.
        .layer(middleware::from_fn(track_metrics))
        // Outermost: assign a request id and a tracing span so every log line
        // for a request is correlated (and the id is echoed to the client).
        .layer(middleware::from_fn(correlation_id))
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
    let body_bytes = axum::body::to_bytes(body, 2 * 1024 * 1024)
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

    let mut response = Json(&gql_response).into_response();
    // Forward HTTP headers set by GraphQL resolvers (e.g. Set-Cookie for refresh tokens)
    for (key, value) in gql_response.http_headers.iter() {
        response.headers_mut().insert(key.clone(), value.clone());
    }
    Ok(response)
}

/// WebSocket handler for GraphQL subscriptions with JWT authentication.
/// Extracts the JWT from the `connection_init` payload and injects Claims into the context.
async fn graphql_ws_handler<Q, M, S>(
    State(state): State<AppState>,
    protocol: GraphQLProtocol,
    upgrade: WebSocketUpgrade,
    schema: Schema<Q, M, S>,
) -> Response
where
    Q: ObjectType + Send + Sync + 'static,
    M: ObjectType + Send + Sync + 'static,
    S: SubscriptionType + Send + Sync + 'static,
{
    let jwt_service = state.jwt_service().clone();

    upgrade
        .protocols(["graphql-transport-ws", "graphql-ws"])
        .on_upgrade(move |stream| {
            GraphQLWebSocket::new(stream, schema, protocol)
                .on_connection_init(move |value: serde_json::Value| {
                    async move {
                        let mut data = async_graphql::Data::default();

                        // Extract token from connectionParams: { headers: { Authorization: "Bearer <token>" } }
                        let token = value
                            .get("headers")
                            .and_then(|h| h.get("Authorization"))
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.strip_prefix("Bearer "));

                        if let Some(token) = token {
                            match jwt_service.verify_token(token) {
                                Ok(claims) => {
                                    data.insert(claims);
                                }
                                Err(_) => {
                                    return Err(async_graphql::Error::new(
                                        "Invalid or expired token",
                                    ));
                                }
                            }
                        }

                        Ok(data)
                    }
                })
                .serve()
        })
}

/// Liveness + quick DB probe.
async fn health(State(state): State<AppState>) -> Result<&'static str, AppError> {
    // Inexpensive round-trip; replace by `SELECT 1` if you prefer.
    let _one: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&state.db).await?;
    Ok("ok")
}

/// Readiness: returns OK only when the database is reachable. Returns 503 (via
/// AppError) otherwise so orchestrators hold traffic until the app can serve.
async fn ready(State(state): State<AppState>) -> Result<&'static str, AppError> {
    let _one: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&state.db).await?;
    Ok("ready")
}

/// Prometheus metrics in text exposition format.
async fn metrics() -> Response {
    (
        [(CONTENT_TYPE, "text/plain; version=0.0.4")],
        render_metrics(),
    )
        .into_response()
}
