//! System endpoints: diagnostics and app backups.
//!
//! - `GET  /api/doctor` — run the same checks as `riku doctor`, as JSON.
//! - `POST /api/apps/:app/backup` — create a backup, return the artifact path.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde_json::json;

use super::mutations::authorize_mutation;
use super::routes::authorize;
use super::DashboardState;
use crate::cli::doctor::{checks, Status};

fn status_str(s: Status) -> &'static str {
    match s {
        Status::Ok => "ok",
        Status::Warn => "warn",
        Status::Fail => "fail",
    }
}

/// GET /api/doctor — diagnostics as a JSON array of `{name, status, detail}`.
pub(crate) async fn doctor(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if let Some(denied) = authorize(&state, &headers, &query) {
        return denied;
    }
    let paths = state.paths.clone();
    let out = tokio::task::spawn_blocking(move || {
        let mut all = Vec::new();
        all.extend(checks::dependencies());
        all.push(checks::directories(&paths));
        all.extend(checks::binary());
        all.push(checks::systemd_service());
        all.extend(checks::nginx());
        all.push(checks::plugins(&paths));
        all.push(checks::disk(&paths));
        all.push(checks::ssh_access());
        all.into_iter()
            .map(|c| {
                json!({ "name": c.name, "status": status_str(c.status), "detail": c.detail })
            })
            .collect::<Vec<_>>()
    })
    .await;

    match out {
        Ok(checks) => Json(checks).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("task failed: {e}")).into_response(),
    }
}

/// POST /api/apps/:app/backup — returns `{artifact: "<path>"}`.
pub(crate) async fn backup_app(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(app): Path<String>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    let app = match crate::util::validate_app_name(&app) {
        Ok(a) => a,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid app name").into_response(),
    };
    let paths = state.paths.clone();
    let result = tokio::task::spawn_blocking(move || {
        crate::deploy::backup::BackupService::new(&paths).backup(&app, None)
    })
    .await;
    match result {
        Ok(Ok(path)) => {
            Json(json!({ "ok": true, "artifact": path.display().to_string() })).into_response()
        }
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("task failed: {e}")).into_response(),
    }
}
