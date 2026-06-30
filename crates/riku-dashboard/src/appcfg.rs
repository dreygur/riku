//! App configuration (environment) endpoints.
//!
//! - `GET  /api/apps/:app/env` — current key/values (read gate).
//! - `POST /api/apps/:app/env` — `{ "set": {K:V}, "unset": [K] }` (token gate).
//!
//! Reads parse the app's `ENV` file directly; writes go through the same
//! `config set` / `config unset` command functions the CLI uses, so validation
//! and on-disk format stay identical.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;

use super::mutations::{authorize_mutation, finish};
use super::routes::authorize;
use super::DashboardState;

/// GET /api/apps/:app/env
pub(crate) async fn get_env(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(app): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if let Some(denied) = authorize(&state, &headers, &query) {
        return denied;
    }
    let app = match crate::util::validate_app_name(&app) {
        Ok(a) => a,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    let mut map: HashMap<String, String> = HashMap::new();
    let _ = crate::util::parse_settings(&state.paths.env_root.join(&app).join("ENV"), &mut map);
    Json(map).into_response()
}

#[derive(Deserialize)]
pub(crate) struct EnvEdit {
    #[serde(default)]
    set: HashMap<String, String>,
    #[serde(default)]
    unset: Vec<String>,
}

/// POST /api/apps/:app/env
pub(crate) async fn edit_env(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(app): Path<String>,
    Json(body): Json<EnvEdit>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    let app = match crate::util::validate_app_name(&app) {
        Ok(a) => a,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid app name").into_response(),
    };

    let paths = state.paths.clone();
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        if !body.set.is_empty() {
            let settings: Vec<String> = body.set.iter().map(|(k, v)| format!("{k}={v}")).collect();
            crate::cli::apps::cmd_config_set(&paths, &app, &settings)?;
        }
        if !body.unset.is_empty() {
            crate::cli::apps::cmd_config_unset(&paths, &app, &body.unset)?;
        }
        Ok(())
    })
    .await;
    finish(result, "config")
}
