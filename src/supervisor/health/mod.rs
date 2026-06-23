//! Health check and metrics HTTP endpoint for monitoring and load balancers.
//!
//! Provides an Axum-based HTTP server that responds to:
//! - GET /health              - Supervisor health status in JSON
//! - GET /metrics             - All process metrics (snapshot from stats.json)
//! - GET /metrics/apps        - Per-app aggregated metrics
//! - GET /metrics/apps/{app}  - Metrics for a specific app
//! - GET /metrics/stream      - Server-Sent Events live metrics broadcast
//! - GET /plugins             - List installed client plugins
//! - GET /hooks                - List installed server-side hook plugins

mod auth;
mod control;
mod plugins;
mod responses;

#[cfg(test)]
mod tests;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::extract::{Extension, Path, State};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use futures::stream::{Stream, StreamExt};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use responses::{build_app_metrics_json, build_health_json, build_metrics_json};

/// Shared context cloned into every Axum handler.
pub struct SharedSupervisorState {
    /// Broadcast sender carrying pre-serialized JSON metrics strings.
    /// The supervisor calls `try_send` after each stats write to push
    /// updates to all connected SSE clients.
    pub metrics_broadcast_tx: broadcast::Sender<String>,
}

/// Start the health check HTTP server on the specified port.
///
/// The server runs on a dedicated Tokio runtime in a background thread and
/// responds to:
/// - GET /health          - JSON containing status, uptime, version
/// - GET /metrics         - JSON metrics snapshot from stats.json
/// - GET /metrics/apps    - Per-app aggregated metrics
/// - GET /metrics/apps/:app - Metrics for a specific app
/// - GET /metrics/stream  - SSE live metrics broadcast
///
/// The TCP listener is bound *synchronously* in this function, before the
/// background thread is spawned, so a bind failure (e.g. the port is already
/// in use) is propagated to the caller as `Err` rather than panicking a
/// detached thread after the function has already returned.
///
/// # Returns
/// * `Ok(broadcast::Sender<String>)` - the broadcast sender the caller uses
///   to publish pre-serialized metrics strings via `try_send`.
/// * `Err` if the TCP listener failed to bind.
pub fn start_health_server(
    port: u16,
    running: Arc<AtomicBool>,
    start_time: SystemTime,
    stats_file: PathBuf,
    control_token_file: PathBuf,
) -> anyhow::Result<broadcast::Sender<String>> {
    let (broadcast_tx, _) = broadcast::channel::<String>(64);

    let state = Arc::new(SharedSupervisorState {
        metrics_broadcast_tx: broadcast_tx.clone(),
    });

    let control_token = auth::load_or_create_token(&control_token_file)
        .map_err(|e| anyhow::anyhow!("failed to load/create control token: {}", e))?;

    let readonly_router = Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/metrics/apps", get(metrics_apps_handler))
        .route("/metrics/apps/:app", get(metrics_app_handler))
        .route("/metrics/stream", get(metrics_stream_handler))
        .route("/plugins", get(plugins::plugins_handler))
        .route("/hooks", get(plugins::hooks_handler))
        .with_state(state)
        .layer(axum::extract::Extension(start_time))
        .layer(axum::extract::Extension(stats_file))
        .layer(readonly_cors_layer());

    // Mutating routes intentionally carry no CorsLayer — only callers that
    // already hold the control token (the dashboard's server-side proxy)
    // are expected to reach them. See `auth` module docs.
    let router = readonly_router.merge(control::control_router(control_token));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    tracing::info!(
        "Health server listening on http://{} (/health, /metrics, /metrics/apps, /metrics/apps/{{app}}, /metrics/stream, /control/apps/*)",
        addr
    );
    tracing::info!("Control-plane token: {}", control_token_file.display());

    // Bind synchronously *before* spawning the worker thread so that a bind
    // failure (e.g. the port is already in use) is surfaced to the caller as
    // an `Err` instead of panicking a detached thread after we've already
    // returned `Ok`. The std listener is then converted to a Tokio listener
    // inside the runtime — that conversion does not re-bind and cannot fail
    // on an already-bound socket.
    let std_listener = std::net::TcpListener::bind(addr)
        .map_err(|e| anyhow::anyhow!("failed to bind health server on {}: {}", addr, e))?;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| anyhow::anyhow!("failed to set health server listener non-blocking: {}", e))?;

    let running_clone = running.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("riku-health")
            .build()
            .expect("failed to create health server Tokio runtime");

        rt.block_on(async move {
            let listener = match tokio::net::TcpListener::from_std(std_listener) {
                Ok(listener) => listener,
                Err(e) => {
                    tracing::error!("failed to adopt health server listener: {}", e);
                    return;
                }
            };

            let shutdown_signal = async move {
                while running_clone.load(Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            };

            // A serve error *after* a clean graceful-shutdown signal must not
            // panic this detached thread — log it and let the thread exit.
            if let Err(e) = axum::serve(listener, router)
                .with_graceful_shutdown(shutdown_signal)
                .await
            {
                tracing::error!("health server stopped with error: {}", e);
            }
        });

        tracing::info!("Health server stopped");
    });

    Ok(broadcast_tx)
}

/// CORS policy for the read-only metrics/health routes.
///
/// Restricts cross-origin reads to the dashboard origin only (configurable via
/// `RIKU_DASHBOARD_ORIGIN`, default `http://127.0.0.1:3000`) instead of the
/// wildcard `Any`. A wildcard origin would let any local web page read app
/// topology from `/metrics/apps/:app`, enabling a DNS-rebinding leak. Methods
/// are limited to the GET/OPTIONS these routes actually serve, and only the
/// `Content-Type` request header is allowed.
fn readonly_cors_layer() -> tower_http::cors::CorsLayer {
    const DEFAULT_ORIGIN: &str = "http://127.0.0.1:3000";

    let configured = std::env::var("RIKU_DASHBOARD_ORIGIN").unwrap_or_default();
    // Fall back to the default on a missing or unparseable origin rather than
    // panicking the supervisor at startup.
    let origin = HeaderValue::from_str(configured.trim())
        .ok()
        .unwrap_or_else(|| HeaderValue::from_static(DEFAULT_ORIGIN));

    tower_http::cors::CorsLayer::new()
        .allow_origin(origin)
        .allow_methods([Method::GET, Method::OPTIONS])
        .allow_headers([axum::http::header::CONTENT_TYPE])
}

// ── Axum Handlers ──────────────────────────────────────────────────────────

/// GET /health — Returns supervisor health status.
async fn health_handler(Extension(start_time): Extension<SystemTime>) -> Json<serde_json::Value> {
    Json(build_health_json(start_time))
}

/// GET /metrics — Returns full metrics snapshot from stats.json.
async fn metrics_handler(
    Extension(stats_file): Extension<PathBuf>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let raw = build_metrics_json(&stats_file);
    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "failed to parse metrics", "detail": e.to_string()})),
        )
    })?;
    Ok(Json(value))
}

/// GET /metrics/apps — Returns per-app aggregated metrics.
async fn metrics_apps_handler(
    Extension(stats_file): Extension<PathBuf>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let raw = build_app_metrics_json(&stats_file, None);
    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "failed to parse app metrics", "detail": e.to_string()})),
        )
    })?;
    Ok(Json(value))
}

/// GET /metrics/apps/:app — Returns metrics for a specific app.
async fn metrics_app_handler(
    Extension(stats_file): Extension<PathBuf>,
    Path(app_name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let raw = build_app_metrics_json(&stats_file, Some(&app_name));

    if raw.starts_with('{') {
        let parsed: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "failed to parse metrics", "detail": e.to_string()})),
            )
        })?;

        if parsed.get("error").and_then(|v| v.as_str()) == Some("Not Found") {
            return Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Not Found",
                    "app": app_name,
                    "message": "No metrics for this app"
                })),
            ));
        }

        return Ok(Json(parsed));
    }

    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "failed to parse metrics", "detail": e.to_string()})),
        )
    })?;
    Ok(Json(value))
}

/// GET /metrics/stream — SSE endpoint broadcasting live metrics.
///
/// Sends the current snapshot immediately on connection, then streams
/// every broadcast update as an `Event::default().data(json)`.
async fn metrics_stream_handler(
    Extension(stats_file): Extension<PathBuf>,
    State(state): State<Arc<SharedSupervisorState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.metrics_broadcast_tx.subscribe();

    // Lagged subscribers are skipped rather than terminated: a slow tab
    // catches back up on the next tick instead of dropping its connection.
    //
    // The broadcast channel carries two kinds of frames: metrics JSON
    // snapshots, and plain deployment notification strings (e.g.
    // "[DEPLOYMENT_FAILED - ROLLING_BACK] ...") pushed by the supervisor's
    // generation orchestrator. Tag them distinctly so clients can tell
    // a rollback notice from a metrics tick without parsing JSON first.
    let updates = BroadcastStream::new(rx).filter_map(|item| async move {
        match item {
            Ok(payload) => Some(Ok(tag_broadcast_event(payload))),
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
                tracing::warn!("SSE subscriber lagged by {} messages, skipping", n);
                None
            }
        }
    });

    let snapshot = build_metrics_json(&stats_file);
    let initial = futures::stream::once(async move {
        Ok(Event::default().event("metrics-update").data(snapshot))
    });

    Sse::new(initial.chain(updates)).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("heartbeat"),
    )
}

/// Tag a raw broadcast payload as either a metrics snapshot or a deployment
/// notification, based on its content (deployment events are always plain
/// bracketed strings, never valid JSON arrays).
fn tag_broadcast_event(payload: String) -> Event {
    if payload.starts_with("[DEPLOYMENT_") {
        Event::default().event("deployment-event").data(payload)
    } else {
        Event::default().event("metrics-update").data(payload)
    }
}
