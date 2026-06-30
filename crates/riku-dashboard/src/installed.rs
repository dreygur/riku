//! Installed-plugin listing endpoint.
//!
//! `GET /api/plugins` → `{ runtimes, hooks, bundles }`:
//! - `runtimes` / `hooks` — executable plugins in `~/.riku/plugins/`, split by
//!   the `riku-` lifecycle-hook prefix.
//! - `bundles` — manifest-based plugin bundles (addons, routers, notifiers).

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
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
            })
        })
        .collect();

    Json(json!({ "runtimes": runtimes, "hooks": hooks, "bundles": bundles })).into_response()
}
