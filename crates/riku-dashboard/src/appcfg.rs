//! App configuration (environment) endpoints.
//!
//! - `GET  /api/apps/:app/env`, current key/values (token gate).
//! - `POST /api/apps/:app/env`, `{ "set": {K:V}, "unset": [K] }` (token gate).
//!
//! Security: the ENV file holds the app's secrets, so reading it needs the
//! same Bearer token as editing it. The open read gate would hand every
//! secret to any local process that can reach loopback.
//!
//! Reads parse the app's `ENV` file directly; writes go through the same
//! `config set` / `config unset` command functions the CLI uses, so validation
//! and on-disk format stay identical.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;

use super::mutations::{authorize_mutation, finish, known_app};
use super::DashboardState;

/// GET /api/apps/:app/env
pub(crate) async fn get_env(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(app): Path<String>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    let app = match crate::util::validate_app_name(&app) {
        Ok(a) => a,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    let mut map: HashMap<String, String> = HashMap::new();
    let env_file = state.paths.env_root.join(&app).join("ENV");
    match crate::util::parse_settings(&env_file, &mut map) {
        Ok(_) => Json(map).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
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
    let app = match known_app(&state.paths, &app) {
        Ok(a) => a,
        Err(refused) => return refused.into_response(),
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

#[cfg(test)]
#[path = "appcfg_tests.rs"]
mod tests;
