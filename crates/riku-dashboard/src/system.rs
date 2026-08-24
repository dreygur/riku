//! System endpoints: diagnostics and app backup/restore.
//!
//! - `GET  /api/doctor`: run the same checks as `riku doctor`, as JSON.
//! - `POST /api/apps/:app/backup`, create a backup, return the artifact path.
//! - `POST /api/apps/:app/restore`, restore from an uploaded `tar.gz`.

use std::collections::HashMap;

use axum::extract::{Multipart, Path, Query, State};
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

/// GET /api/doctor: diagnostics as a JSON array of `{name, status, detail}`.
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
            .map(|c| json!({ "name": c.name, "status": status_str(c.status), "detail": c.detail }))
            .collect::<Vec<_>>()
    })
    .await;

    match out {
        Ok(checks) => Json(checks).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("task failed: {e}"),
        )
            .into_response(),
    }
}

/// POST /api/apps/:app/backup, returns `{artifact: "<path>"}`.
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
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("task failed: {e}"),
        )
            .into_response(),
    }
}

/// POST /api/apps/:app/restore, multipart upload, field name `file`, the
/// `tar.gz` produced by `riku backup` / `POST .../backup`.
///
/// The upload is written to a scratch file under `data_root` (never a
/// caller-controlled path) and removed again once
/// [`crate::deploy::backup::BackupService::restore`] returns, that function
/// does its own archive-member validation (no absolute paths, no `..`, every
/// entry confined to this app's directories) before extracting anything.
pub(crate) async fn restore_app(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(app): Path<String>,
    mut multipart: Multipart,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    let app = match crate::util::validate_app_name(&app) {
        Ok(a) => a,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid app name").into_response(),
    };

    let bytes = loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => return (StatusCode::BAD_REQUEST, "missing 'file' field").into_response(),
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("invalid upload: {e}")).into_response()
            }
        };
        if field.name() != Some("file") {
            continue;
        }
        match field.bytes().await {
            Ok(b) => break b,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("invalid upload: {e}")).into_response()
            }
        }
    };

    let paths = state.paths.clone();
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let scratch_dir = paths.data_root.join("tmp-restores");
        std::fs::create_dir_all(&scratch_dir)?;
        let scratch_path = scratch_dir.join(format!("{app}.restore.tar.gz"));
        std::fs::write(&scratch_path, &bytes)?;
        let result = crate::deploy::backup::BackupService::new(&paths).restore(&app, &scratch_path);
        let _ = std::fs::remove_file(&scratch_path);
        result
    })
    .await;

    match result {
        Ok(Ok(())) => Json(json!({ "ok": true })).into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("task failed: {e}"),
        )
            .into_response(),
    }
}
