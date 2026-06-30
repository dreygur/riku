//! Addon (managed datastore) endpoints — Plugin Protocol v1 addon seam.
//!
//! - `GET    /api/addons` — provisioned instances (read gate).
//! - `POST   /api/addons` — `{plugin, instance}` provision (token gate).
//! - `POST   /api/addons/:instance/bind`   — `{app}` → injected env keys.
//! - `POST   /api/addons/:instance/unbind` — `{app}`.
//! - `POST   /api/addons/:instance/backup` — `{artifact}`.
//! - `DELETE /api/addons/:instance` — deprovision (guarded by the service).
//!
//! All of it runs through [`AddonService`], the same logic the CLI uses.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;
use serde_json::json;

use super::mutations::authorize_mutation;
use super::routes::authorize;
use super::DashboardState;
use crate::plugins::AddonService;

/// GET /api/addons
pub(crate) async fn list(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if let Some(denied) = authorize(&state, &headers, &query) {
        return denied;
    }
    let instances = AddonService::new(&state.paths).list();
    Json(instances).into_response()
}

#[derive(Deserialize)]
pub(crate) struct CreateBody {
    plugin: String,
    instance: String,
}

/// POST /api/addons
pub(crate) async fn create(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Json(body): Json<CreateBody>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    blocking(state, move |paths| {
        AddonService::new(paths).provision(&body.plugin, &body.instance)?;
        Ok(json!({ "ok": true, "instance": body.instance }))
    })
    .await
}

#[derive(Deserialize)]
pub(crate) struct AppBody {
    app: String,
}

/// POST /api/addons/:instance/bind
pub(crate) async fn bind(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(instance): Path<String>,
    Json(body): Json<AppBody>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    blocking(state, move |paths| {
        let keys = AddonService::new(paths).bind(&instance, &body.app)?;
        Ok(json!({ "ok": true, "injected": keys }))
    })
    .await
}

/// POST /api/addons/:instance/unbind
pub(crate) async fn unbind(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(instance): Path<String>,
    Json(body): Json<AppBody>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    blocking(state, move |paths| {
        AddonService::new(paths).unbind(&instance, &body.app)?;
        Ok(json!({ "ok": true }))
    })
    .await
}

/// POST /api/addons/:instance/backup
pub(crate) async fn backup(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(instance): Path<String>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    blocking(state, move |paths| {
        let artifact = AddonService::new(paths).backup(&instance)?;
        Ok(json!({ "ok": true, "artifact": artifact }))
    })
    .await
}

/// DELETE /api/addons/:instance
pub(crate) async fn deprovision(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(instance): Path<String>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    blocking(state, move |paths| {
        AddonService::new(paths).deprovision(&instance)?;
        Ok(json!({ "ok": true }))
    })
    .await
}

/// Run a blocking addon operation that returns a JSON body, shaping the result.
async fn blocking<F>(state: DashboardState, f: F) -> Response
where
    F: FnOnce(&crate::config::RikuPaths) -> anyhow::Result<serde_json::Value> + Send + 'static,
{
    let paths = state.paths.clone();
    match tokio::task::spawn_blocking(move || f(&paths)).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("task failed: {e}")).into_response(),
    }
}
