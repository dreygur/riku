//! Mutating control-plane routes: create / deploy / restart / stop / destroy.
//!
//! Every route here changes on-disk app state or running processes, so each
//! request must carry `Authorization: Bearer <control_token_file contents>`
//! (see [`super::auth`]). Handlers reuse the existing CLI command functions
//! in `crate::cli::apps` rather than re-implementing the logic, so behavior
//! stays identical to running `riku <cmd>` directly.

use std::sync::Arc;

use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Json};
use axum::routing::{delete, post};
use axum::Router;
use serde_json::{json, Value};

use crate::config::RikuPaths;

#[derive(Clone)]
pub struct ControlState {
    pub token: Arc<String>,
}

/// Build the mutating control-plane sub-router, gated by bearer-token auth.
/// Deliberately has no `CorsLayer` — only the dashboard's own server-side
/// proxy (or another local process holding the token) is expected to call
/// these routes, never a browser directly.
pub fn control_router(token: String) -> Router {
    let state = ControlState {
        token: Arc::new(token),
    };

    Router::new()
        .route("/control/apps", post(create_app_handler))
        .route("/control/apps/:app/deploy", post(deploy_handler))
        .route("/control/apps/:app/restart", post(restart_handler))
        .route("/control/apps/:app/stop", post(stop_handler))
        .route("/control/apps/:app", delete(destroy_handler))
        .route("/control/plugins/install", post(install_plugins_handler))
        .route(
            "/control/apps/:app/container/export",
            post(container_export_handler),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), require_token))
        .with_state(state)
}

/// Rejects any request whose `Authorization: Bearer <token>` header doesn't
/// match the control token, before it reaches a handler.
async fn require_token(
    State(state): State<ControlState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> impl IntoResponse {
    let provided = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match provided {
        Some(token) if super::auth::constant_time_eq(token, &state.token) => {
            next.run(request).await
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized", "message": "missing or invalid control token"})),
        )
            .into_response(),
    }
}

type HandlerResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

fn run_blocking_command(
    label: &'static str,
    app: String,
    f: impl FnOnce(&RikuPaths, &str) -> anyhow::Result<()> + Send + 'static,
) -> std::thread::JoinHandle<HandlerResult> {
    std::thread::spawn(move || {
        let paths = RikuPaths::from_env();
        match f(&paths, &app) {
            Ok(()) => Ok(Json(json!({"ok": true, "app": app, "action": label}))),
            Err(e) => {
                // A deploy already in progress for this app is a conflict,
                // not a server failure — surface 409 so callers can retry
                // instead of treating it like a crash.
                let status = match e.downcast_ref::<crate::error::DeployError>() {
                    Some(crate::error::DeployError::DeployInProgress(_)) => StatusCode::CONFLICT,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                };
                Err((
                    status,
                    Json(json!({"ok": false, "app": app, "action": label, "error": e.to_string()})),
                ))
            }
        }
    })
}

async fn join_blocking(handle: std::thread::JoinHandle<HandlerResult>) -> impl IntoResponse {
    match tokio::task::spawn_blocking(move || handle.join())
        .await
        .expect("blocking task panicked")
    {
        Ok(result) => result.into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "error": "control action thread panicked"})),
        )
            .into_response(),
    }
}

/// POST /control/apps — { "name": "myapp" }
///
/// `cmd_apps_create` sanitizes `name` (stripping spaces/punctuation — see
/// `validate_app_name`), so the app actually created on disk can differ
/// from the request body. The response's "app" field must reflect that
/// sanitized name, not the raw input, or callers (the dashboard) end up
/// polling a name that was never created.
async fn create_app_handler(Json(body): Json<Value>) -> impl IntoResponse {
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"ok": false, "error": "missing required field 'name'"})),
            )
                .into_response();
        }
    };

    let handle = std::thread::spawn(move || {
        let paths = RikuPaths::from_env();
        match crate::cli::apps::cmd_apps_create(&paths, &name) {
            Ok(sanitized) => Ok(Json(
                json!({"ok": true, "app": sanitized, "action": "create"}),
            )),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"ok": false, "app": name, "action": "create", "error": e.to_string()})),
            )),
        }
    });
    join_blocking(handle).await.into_response()
}

/// POST /control/apps/:app/deploy
async fn deploy_handler(Path(app): Path<String>) -> impl IntoResponse {
    let handle = run_blocking_command("deploy", app, |paths, app| {
        crate::cli::apps::cmd_deploy(paths, app, None)
    });
    join_blocking(handle).await.into_response()
}

/// POST /control/apps/:app/restart
async fn restart_handler(Path(app): Path<String>) -> impl IntoResponse {
    let handle = run_blocking_command("restart", app, |paths, app| {
        crate::cli::apps::cmd_restart(paths, app)
    });
    join_blocking(handle).await.into_response()
}

/// POST /control/apps/:app/stop
async fn stop_handler(Path(app): Path<String>) -> impl IntoResponse {
    let handle = run_blocking_command("stop", app, |paths, app| {
        crate::cli::apps::cmd_stop(paths, app)
    });
    join_blocking(handle).await.into_response()
}

/// DELETE /control/apps/:app
async fn destroy_handler(Path(app): Path<String>) -> impl IntoResponse {
    let handle = run_blocking_command("destroy", app, |paths, app| {
        crate::cli::apps::cmd_destroy(paths, app)
    });
    join_blocking(handle).await.into_response()
}

/// POST /control/plugins/install — { "only": ["node", "python"] } (omit for all bundled runtimes)
async fn install_plugins_handler(body: Option<Json<Value>>) -> impl IntoResponse {
    let only: Option<Vec<String>> = body
        .as_ref()
        .and_then(|b| b.get("only"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        });

    let handle = std::thread::spawn(move || -> HandlerResult {
        let paths = RikuPaths::from_env();
        match crate::cli::apps::cmd_install_plugins(&paths, only) {
            Ok(()) => Ok(Json(json!({"ok": true, "action": "install-plugins"}))),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"ok": false, "action": "install-plugins", "error": e.to_string()})),
            )),
        }
    });
    join_blocking(handle).await.into_response()
}

/// POST /control/apps/:app/container/export — builds the app's deployed
/// source directory as a container image (auto-detects Docker/Podman) and
/// exports it to a tar archive under `{data_root}/exports/{app}.tar`.
///
/// The build context and output path are always server-derived from the
/// validated app name — never taken from the request — so this can't be
/// used to build an arbitrary host directory or write outside riku's data
/// root.
async fn container_export_handler(Path(app): Path<String>) -> impl IntoResponse {
    let handle = std::thread::spawn(move || -> HandlerResult {
        let paths = RikuPaths::from_env();

        let app = match crate::util::validate_app_name(&app) {
            Ok(a) => a,
            Err(e) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"ok": false, "error": e.to_string()})),
                ))
            }
        };

        let context = paths.app_root.join(&app);
        if !context.exists() {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"ok": false, "app": app, "error": format!("app '{}' not found", app)})),
            ));
        }

        let export_dir = paths.data_root.join("exports");
        if let Err(e) = std::fs::create_dir_all(&export_dir) {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"ok": false, "app": app, "error": e.to_string()})),
            ));
        }

        let output = export_dir.join(format!("{}.tar", app));
        match crate::deploy::container_runtime::build_and_export(&app, &context, &output) {
            Ok(()) => Ok(Json(json!({
                "ok": true,
                "app": app,
                "action": "container-export",
                "output": output.display().to_string(),
            }))),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "ok": false,
                    "app": app,
                    "action": "container-export",
                    "error": e.to_string(),
                })),
            )),
        }
    });
    join_blocking(handle).await.into_response()
}
