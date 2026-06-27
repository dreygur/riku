//! Dashboard mutating actions (Track A, Phase 1 — second half).
//!
//! Restart / stop / redeploy, gated harder than the read-only API:
//! - A token is **required** (the dashboard must be started with `--token`);
//!   loopback openness does not apply to mutations.
//! - The token must arrive in the `Authorization: Bearer` header — never a query
//!   param. A cross-origin page cannot set that header without a CORS preflight,
//!   which this server never approves, so this doubles as CSRF protection.
//! - Each action reuses the same service function the CLI calls, on a blocking
//!   task so the reactor is not stalled.

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::post;
use axum::Router;

use super::DashboardState;

pub(crate) fn router() -> Router<DashboardState> {
    Router::new()
        .route("/api/apps/:app/restart", post(restart))
        .route("/api/apps/:app/stop", post(stop))
        .route("/api/apps/:app/redeploy", post(redeploy))
        .route("/api/apps/:app/scale", post(scale))
        .route("/api/apps/:app/rollback", post(rollback))
}

async fn restart(state: State<DashboardState>, headers: HeaderMap, app: Path<String>) -> Response {
    run_action(state, headers, app, "restart", |paths, app| {
        crate::cli::apps::cmd_restart(paths, app)
    })
    .await
}

async fn stop(state: State<DashboardState>, headers: HeaderMap, app: Path<String>) -> Response {
    run_action(state, headers, app, "stop", |paths, app| {
        crate::cli::apps::cmd_stop(paths, app)
    })
    .await
}

async fn redeploy(state: State<DashboardState>, headers: HeaderMap, app: Path<String>) -> Response {
    run_action(state, headers, app, "redeploy", |paths, app| {
        crate::cli::apps::cmd_deploy(paths, app, None)
    })
    .await
}

/// POST /api/apps/:app/scale — body `{"web":2,"worker":1}` (kind → count).
async fn scale(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(app): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    let app = match crate::util::validate_app_name(&app) {
        Ok(a) => a,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid app name").into_response(),
    };
    // Translate the JSON object into the CLI's `kind=count` settings.
    let settings: Vec<String> = body
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_u64().map(|n| format!("{k}={n}")))
                .collect()
        })
        .unwrap_or_default();
    if settings.is_empty() {
        return (StatusCode::BAD_REQUEST, "no scale settings provided").into_response();
    }

    let paths = state.paths.clone();
    let result =
        tokio::task::spawn_blocking(move || crate::cli::apps::cmd_ps_scale(&paths, &app, &settings))
            .await;
    finish(result, "scale")
}

/// POST /api/apps/:app/rollback — body `{"to":"<sha>"}` (omit `to` for previous).
async fn rollback(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(app): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    let app = match crate::util::validate_app_name(&app) {
        Ok(a) => a,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid app name").into_response(),
    };
    let to = body
        .get("to")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let paths = state.paths.clone();
    let result = tokio::task::spawn_blocking(move || {
        crate::deploy::rollback(&app, &paths, to.as_deref())
    })
    .await;
    finish(result, "rollback")
}

/// Shape a `spawn_blocking` result into the standard action response.
fn finish(
    result: Result<anyhow::Result<()>, tokio::task::JoinError>,
    name: &'static str,
) -> Response {
    match result {
        Ok(Ok(())) => Json(serde_json::json!({ "ok": true, "action": name })).into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("task failed: {e}")).into_response(),
    }
}

/// Authorize, validate the app name, and run `action` on a blocking task.
async fn run_action(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(app): Path<String>,
    name: &'static str,
    action: fn(&crate::config::RikuPaths, &str) -> anyhow::Result<()>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    let app = match crate::util::validate_app_name(&app) {
        Ok(a) => a,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid app name").into_response(),
    };

    let paths = state.paths.clone();
    let result = tokio::task::spawn_blocking(move || action(&paths, &app)).await;

    match result {
        Ok(Ok(())) => Json(serde_json::json!({ "ok": true, "action": name })).into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("task failed: {e}"),
        )
            .into_response(),
    }
}

/// Require the configured token in the `Authorization: Bearer` header.
fn authorize_mutation(state: &DashboardState, headers: &HeaderMap) -> Option<Response> {
    let Some(expected) = &state.token else {
        return Some(
            (
                StatusCode::FORBIDDEN,
                "mutating actions are disabled — start the dashboard with --token",
            )
                .into_response(),
        );
    };
    let provided = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let ok = provided
        .map(|tok| crate::supervisor::health::auth::constant_time_eq(tok, expected))
        .unwrap_or(false);
    if ok {
        None
    } else {
        Some((StatusCode::UNAUTHORIZED, "missing or invalid token").into_response())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RikuPaths;

    fn state(token: Option<&str>) -> DashboardState {
        DashboardState {
            paths: RikuPaths::from_dirs("/tmp/riku-mut-test".into(), std::path::Path::new("/tmp")),
            token: token.map(str::to_string),
        }
    }

    fn auth_header(value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, value.parse().unwrap());
        h
    }

    #[test]
    fn mutations_disabled_without_a_configured_token() {
        assert!(authorize_mutation(&state(None), &auth_header("Bearer x")).is_some());
    }

    #[test]
    fn requires_matching_bearer_token() {
        let st = state(Some("secret"));
        assert!(authorize_mutation(&st, &auth_header("Bearer secret")).is_none());
        assert!(authorize_mutation(&st, &auth_header("Bearer nope")).is_some());
        assert!(authorize_mutation(&st, &HeaderMap::new()).is_some());
    }
}
