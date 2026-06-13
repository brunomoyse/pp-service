//! Observability: logging setup, request correlation IDs, and a minimal
//! Prometheus metrics surface.
//!
//! Metrics are hand-rolled with atomics (no extra dependency) — enough for an
//! ops dashboard (request volume, status mix, in-flight, uptime) without
//! pulling in a metrics framework. Swap for the `metrics` crate if richer
//! instrumentation (histograms, per-route labels) is needed later.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use axum::{
    extract::Request,
    http::{header::HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

static REQUESTS_TOTAL: AtomicU64 = AtomicU64::new(0);
static RESPONSES_2XX: AtomicU64 = AtomicU64::new(0);
static RESPONSES_3XX: AtomicU64 = AtomicU64::new(0);
static RESPONSES_4XX: AtomicU64 = AtomicU64::new(0);
static RESPONSES_5XX: AtomicU64 = AtomicU64::new(0);
static IN_FLIGHT: AtomicI64 = AtomicI64::new(0);
static START: OnceLock<Instant> = OnceLock::new();

const REQUEST_ID_HEADER: &str = "x-request-id";

/// Initialize tracing. `LOG_FORMAT=json` emits structured JSON lines (for log
/// aggregation in production); anything else keeps the human-readable format.
/// Also stamps the process start instant for the uptime metric.
pub fn init_tracing() {
    let _ = START.set(Instant::now());

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let json = std::env::var("LOG_FORMAT")
        .map(|v| v == "json")
        .unwrap_or(false);

    let registry = tracing_subscriber::registry().with(filter);
    if json {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }
}

/// Middleware: attach a request id (honor an inbound `x-request-id`, else mint
/// one), put it on a tracing span covering the request so every log line for
/// the request carries it, and echo it back on the response.
pub async fn correlation_id(req: Request, next: Next) -> Response {
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let span = tracing::info_span!(
        "request",
        request_id = %request_id,
        method = %req.method(),
        path = %req.uri().path(),
    );

    let mut response = next.run(req).instrument(span).await;

    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
    }
    response
}

/// Middleware: count requests, responses by status class, and in-flight.
pub async fn track_metrics(req: Request, next: Next) -> Response {
    REQUESTS_TOTAL.fetch_add(1, Ordering::Relaxed);
    IN_FLIGHT.fetch_add(1, Ordering::Relaxed);

    let response = next.run(req).await;

    IN_FLIGHT.fetch_sub(1, Ordering::Relaxed);
    match response.status().as_u16() {
        200..=299 => RESPONSES_2XX.fetch_add(1, Ordering::Relaxed),
        300..=399 => RESPONSES_3XX.fetch_add(1, Ordering::Relaxed),
        400..=499 => RESPONSES_4XX.fetch_add(1, Ordering::Relaxed),
        _ => RESPONSES_5XX.fetch_add(1, Ordering::Relaxed),
    };

    response
}

/// Render the current metrics in Prometheus text exposition format.
pub fn render_metrics() -> String {
    let uptime = START.get().map(|s| s.elapsed().as_secs()).unwrap_or(0);
    let mut out = String::with_capacity(512);

    out.push_str("# HELP pocketpair_http_requests_total Total HTTP requests received.\n");
    out.push_str("# TYPE pocketpair_http_requests_total counter\n");
    out.push_str(&format!(
        "pocketpair_http_requests_total {}\n",
        REQUESTS_TOTAL.load(Ordering::Relaxed)
    ));

    out.push_str("# HELP pocketpair_http_responses_total HTTP responses by status class.\n");
    out.push_str("# TYPE pocketpair_http_responses_total counter\n");
    for (class, counter) in [
        ("2xx", &RESPONSES_2XX),
        ("3xx", &RESPONSES_3XX),
        ("4xx", &RESPONSES_4XX),
        ("5xx", &RESPONSES_5XX),
    ] {
        out.push_str(&format!(
            "pocketpair_http_responses_total{{class=\"{}\"}} {}\n",
            class,
            counter.load(Ordering::Relaxed)
        ));
    }

    out.push_str("# HELP pocketpair_http_requests_in_flight Requests currently being served.\n");
    out.push_str("# TYPE pocketpair_http_requests_in_flight gauge\n");
    out.push_str(&format!(
        "pocketpair_http_requests_in_flight {}\n",
        IN_FLIGHT.load(Ordering::Relaxed).max(0)
    ));

    out.push_str("# HELP pocketpair_uptime_seconds Process uptime in seconds.\n");
    out.push_str("# TYPE pocketpair_uptime_seconds gauge\n");
    out.push_str(&format!("pocketpair_uptime_seconds {uptime}\n"));

    out
}
