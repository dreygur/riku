//! Dashboard HTTP routes: one embedded page plus a read-only JSON API.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Json, Response};
use axum::routing::{delete, get, post};
use axum::Router;

use super::DashboardState;

/// The dashboard UI, embedded into the binary at compile time.
const INDEX_HTML: &str = include_str!("index.html");

pub(crate) fn router(state: DashboardState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/healthz", get(healthz))
        .route("/api/state", get(api_state))
        .route("/api/apps/:app/releases", get(api_releases))
        .route("/api/apps/:app/logs", get(super::logs::stream))
        // env editor
        .route(
            "/api/apps/:app/env",
            get(super::appcfg::get_env).post(super::appcfg::edit_env),
        )
        // app backup + diagnostics
        .route("/api/apps/:app/backup", post(super::system::backup_app))
        .route("/api/doctor", get(super::system::doctor))
        // addons (managed datastores)
        .route(
            "/api/addons",
            get(super::addons::list).post(super::addons::create),
        )
        .route("/api/addons/:instance", delete(super::addons::deprovision))
        .route("/api/addons/:instance/bind", post(super::addons::bind))
        .route("/api/addons/:instance/unbind", post(super::addons::unbind))
        .route("/api/addons/:instance/backup", post(super::addons::backup))
        .merge(super::mutations::router())
        .with_state(state)
}

async fn index() -> impl IntoResponse {
    Html(INDEX_HTML)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn api_state(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if let Some(denied) = authorize(&state, &headers, &query) {
        return denied;
    }
    match crate::cli::apps::state_json(&state.paths) {
        Ok(value) => Json(value).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response(),
    }
}

/// GET /api/apps/:app/releases — recorded deploy history (for the rollback UI).
async fn api_releases(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    axum::extract::Path(app): axum::extract::Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if let Some(denied) = authorize(&state, &headers, &query) {
        return denied;
    }
    let app = match crate::util::validate_app_name(&app) {
        Ok(a) => a,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    let releases: Vec<_> = crate::deploy::releases::ReleaseLog::new(&state.paths)
        .list(&app)
        .into_iter()
        .map(|r| serde_json::json!({ "ts": r.ts, "sha": r.sha }))
        .collect();
    Json(releases).into_response()
}

/// Enforce the Host allowlist (DNS-rebinding guard) and the API token. Returns
/// `Some(response)` to deny, `None` to allow.
pub(crate) fn authorize(
    state: &DashboardState,
    headers: &HeaderMap,
    query: &HashMap<String, String>,
) -> Option<Response> {
    // Without a token we only serve loopback Hosts, so a rebinding browser
    // page pointed at 127.0.0.1 with an attacker Host can't read the API.
    if state.token.is_none() && !host_is_loopback(headers) {
        return Some((StatusCode::FORBIDDEN, "non-loopback Host not allowed").into_response());
    }

    if let Some(expected) = &state.token {
        let provided = bearer(headers).or_else(|| query.get("token").map(String::as_str));
        let ok = provided
            .map(|tok| crate::supervisor::health::auth::constant_time_eq(tok, expected))
            .unwrap_or(false);
        if !ok {
            return Some((StatusCode::UNAUTHORIZED, "missing or invalid token").into_response());
        }
    }
    None
}

fn host_is_loopback(headers: &HeaderMap) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
        // No Host header (e.g. HTTP/1.0) — treat as local.
        return true;
    };
    let hostname = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
    matches!(hostname, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RikuPaths;

    fn state(token: Option<&str>) -> DashboardState {
        DashboardState {
            paths: RikuPaths::from_dirs("/tmp/riku-dash-test".into(), std::path::Path::new("/tmp")),
            token: token.map(str::to_string),
        }
    }

    fn headers_with_host(host: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, host.parse().unwrap());
        h
    }

    #[test]
    fn loopback_host_allowed_without_token() {
        let q = HashMap::new();
        assert!(authorize(&state(None), &headers_with_host("localhost:8088"), &q).is_none());
        assert!(authorize(&state(None), &headers_with_host("127.0.0.1"), &q).is_none());
    }

    #[test]
    fn non_loopback_host_rejected_without_token() {
        let q = HashMap::new();
        assert!(authorize(&state(None), &headers_with_host("evil.example.com"), &q).is_some());
    }

    #[test]
    fn token_required_and_checked_when_set() {
        let st = state(Some("secret"));
        let host = headers_with_host("evil.example.com"); // host check skipped when token set
        let mut wrong = HashMap::new();
        wrong.insert("token".to_string(), "nope".to_string());
        assert!(authorize(&st, &host, &wrong).is_some());

        let mut right = HashMap::new();
        right.insert("token".to_string(), "secret".to_string());
        assert!(authorize(&st, &host, &right).is_none());
    }
}
