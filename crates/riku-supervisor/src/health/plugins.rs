//! Read-only plugin/hook discovery routes.
//!
//! Surfaces the same data as `riku plugin list` and `riku hook list`,
//! via the injected control-plane actions
//! and `crate::plugins` so the dashboard never drifts from CLI behavior.

use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::Json;
use serde_json::{json, Value};

use super::actions::SharedActions;
use riku_config::RikuPaths;

/// GET /plugins — client-side plugins (`~/.riku/client-plugins/`).
pub(super) async fn plugins_handler(
    Extension(actions): Extension<SharedActions>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match actions.list_client_plugins() {
        Ok(plugins) => Ok(Json(json!({ "plugins": plugins }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "failed to list client plugins", "detail": e.to_string()})),
        )),
    }
}

/// GET /hooks — server-side lifecycle hook plugins (`~/.riku/plugins/`).
pub(super) async fn hooks_handler() -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let paths = RikuPaths::from_env();
    match crate::plugins::list_plugins(&paths) {
        Ok(hooks) => Ok(Json(json!({ "hooks": hooks }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "failed to list hook plugins", "detail": e.to_string()})),
        )),
    }
}
