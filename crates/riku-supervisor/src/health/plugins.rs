//! Read-only hook plugin discovery route.
//!
//! Surfaces the same data as `riku hook list` through `crate::plugins`, so
//! the dashboard never drifts from CLI behavior.

use axum::http::StatusCode;
use axum::response::Json;
use serde_json::{json, Value};

use riku_config::RikuPaths;

/// GET /hooks: server-side lifecycle hook plugins (`~/.riku/plugins/`).
pub(super) async fn hooks_handler() -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let paths = match RikuPaths::from_env() {
        Ok(paths) => paths,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to resolve riku paths", "detail": e.to_string()})),
            ))
        }
    };
    match crate::plugins::list_plugins(&paths) {
        Ok(hooks) => Ok(Json(json!({ "hooks": hooks }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "failed to list hook plugins", "detail": e.to_string()})),
        )),
    }
}
