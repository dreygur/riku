//! Dashboard mutating actions on container services.

use axum::extract::{Json, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use super::mutations::{authorize_mutation, finish};
use super::DashboardState;
use crate::deploy::containers::{
    restart_service, scale_service, stop_service, AppName, ServiceName,
};

pub(crate) async fn restart(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path((app, service)): Path<(String, String)>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    let app = match AppName::try_from(app.as_str()) {
        Ok(a) => a,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid app name").into_response(),
    };
    let service = match ServiceName::try_from(service.as_str()) {
        Ok(s) => s,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid service name").into_response(),
    };
    let paths = state.paths.clone();
    let result =
        tokio::task::spawn_blocking(move || restart_service(&app, &paths, Some(&service))).await;
    finish(result, "restart_service")
}

pub(crate) async fn stop(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path((app, service)): Path<(String, String)>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    let app = match AppName::try_from(app.as_str()) {
        Ok(a) => a,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid app name").into_response(),
    };
    let service = match ServiceName::try_from(service.as_str()) {
        Ok(s) => s,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid service name").into_response(),
    };
    let paths = state.paths.clone();
    let result =
        tokio::task::spawn_blocking(move || stop_service(&app, &paths, Some(&service))).await;
    finish(result, "stop_service")
}

pub(crate) async fn scale(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path((app, service)): Path<(String, String)>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    let app = match AppName::try_from(app.as_str()) {
        Ok(a) => a,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid app name").into_response(),
    };
    let service = match ServiceName::try_from(service.as_str()) {
        Ok(s) => s,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid service name").into_response(),
    };
    let copies = match body.get("copies").and_then(|v| v.as_u64()) {
        Some(n) => n as u32,
        None => return (StatusCode::BAD_REQUEST, "missing copies").into_response(),
    };
    let paths = state.paths.clone();
    let result =
        tokio::task::spawn_blocking(move || scale_service(&app, &paths, &service, copies)).await;
    finish(result, "scale_service")
}

#[cfg(test)]
#[path = "service_mutations_tests.rs"]
mod tests;
