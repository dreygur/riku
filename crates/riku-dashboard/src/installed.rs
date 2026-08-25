//! Installed-plugin listing endpoint.
//!
//! `GET /api/plugins` → `{ runtimes, hooks, bundles }`:
//! - `runtimes` / `hooks`: executable plugins in `~/.riku/plugins/`, split by
//!   the `riku-` lifecycle-hook prefix.
//! - `bundles`: manifest-based plugin bundles (addons, routers, notifiers).
//!
//! `GET /api/plugins/:name/ui`, dispatches that plugin's `ui_panel` verb
//! (Plugin Protocol v1 §7.5) and returns the structured panel JSON. Only
//! plugins declaring `[ui]` (i.e. present with a `ui.nav_label` in the list
//! above) are meaningful to call this for, but calling it for any other
//! installed plugin just gets an empty panel back (`run_ui_panel` degrades
//! safely rather than erroring): a nonexistent plugin name is the one real
//! 404 case.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde_json::json;

use super::routes::authorize;
use super::DashboardState;

/// GET /api/plugins
pub(crate) async fn list(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if let Some(denied) = authorize(&state, &headers, &query) {
        return denied;
    }

    let execs = crate::plugins::list_plugins(&state.paths).unwrap_or_default();
    let (hooks, runtimes): (Vec<String>, Vec<String>) =
        execs.into_iter().partition(|n| n.starts_with("riku-"));

    let bundles: Vec<_> = crate::plugins::bundles::find_bundles(&state.paths.plugin_root)
        .into_iter()
        .map(|(_, m)| {
            json!({
                "name": m.name,
                "version": m.version,
                "type": format!("{:?}", m.plugin_type).to_lowercase(),
                "description": m.description,
                "author": m.author,
                "ui": m.ui.nav_label.as_ref().map(|nav_label| json!({ "nav_label": nav_label })),
            })
        })
        .collect();

    Json(json!({ "runtimes": runtimes, "hooks": hooks, "bundles": bundles })).into_response()
}

/// GET /api/plugins/:name/ui
pub(crate) async fn ui_panel(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Path(name): Path<String>,
) -> Response {
    if let Some(denied) = authorize(&state, &headers, &query) {
        return denied;
    }

    let paths = state.paths.clone();
    let found = tokio::task::spawn_blocking(move || {
        crate::plugins::bundles::find_plugin(&paths.plugin_root, &name)
            .map(|(bundle, manifest)| (paths, bundle, manifest))
    })
    .await;

    let (paths, bundle, manifest) = match found {
        Ok(Some(found)) => found,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                json!({"error": "plugin not found"}).to_string(),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("task failed: {e}"),
            )
                .into_response()
        }
    };

    let panel = tokio::task::spawn_blocking(move || {
        crate::plugins::run_ui_panel(&paths, &bundle, &manifest)
    })
    .await;

    match panel {
        Ok(panel) => Json(panel).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("task failed: {e}"),
        )
            .into_response(),
    }
}
