//! Plugin distribution endpoints: marketplace sources, search, install/remove,
//! and the author-signature trust keyring.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;
use serde_json::{json, Value};

use super::mutations::authorize_mutation;
use super::routes::authorize;
use super::DashboardState;
use crate::plugins::{signing::Keyring, MarketplaceService, PluginInstaller};

/// Run a blocking op returning a JSON body; shape the result.
async fn blocking<F>(state: DashboardState, f: F) -> Response
where
    F: FnOnce(&crate::config::RikuPaths) -> anyhow::Result<Value> + Send + 'static,
{
    let paths = state.paths.clone();
    match tokio::task::spawn_blocking(move || f(&paths)).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("task failed: {e}")).into_response(),
    }
}

// ---- marketplace ----

/// GET /api/marketplace — registered sources.
pub(crate) async fn list_sources(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if let Some(denied) = authorize(&state, &headers, &query) {
        return denied;
    }
    Json(MarketplaceService::new(&state.paths).list()).into_response()
}

/// GET /api/marketplace/search?q= — entries across sources.
pub(crate) async fn search(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if let Some(denied) = authorize(&state, &headers, &query) {
        return denied;
    }
    let q = query.get("q").map(String::as_str).unwrap_or("");
    let hits: Vec<_> = MarketplaceService::new(&state.paths)
        .search(q)
        .into_iter()
        .map(|(market, e)| {
            json!({
                "marketplace": market,
                "name": e.name,
                "source": e.source,
                "description": e.description,
            })
        })
        .collect();
    Json(hits).into_response()
}

#[derive(Deserialize)]
pub(crate) struct AddSource {
    url: String,
    name: Option<String>,
}

/// POST /api/marketplace — add a source `{url, name?}`.
pub(crate) async fn add_source(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Json(body): Json<AddSource>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    blocking(state, move |paths| {
        let name = MarketplaceService::new(paths).add(&body.url, body.name.as_deref())?;
        Ok(json!({ "ok": true, "name": name }))
    })
    .await
}

/// DELETE /api/marketplace/:name — remove a source.
pub(crate) async fn remove_source(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    blocking(state, move |paths| {
        MarketplaceService::new(paths).remove(&name)?;
        Ok(json!({ "ok": true }))
    })
    .await
}

// ---- install / remove a plugin bundle ----

#[derive(Deserialize)]
pub(crate) struct InstallBody {
    source: String,
}

/// POST /api/plugins/install — install a bundle from a source string.
pub(crate) async fn install(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Json(body): Json<InstallBody>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    blocking(state, move |paths| {
        let m = PluginInstaller::new(paths).install(&body.source)?;
        Ok(json!({ "ok": true, "name": m.name, "version": m.version }))
    })
    .await
}

/// DELETE /api/plugins/:name — remove an installed bundle.
pub(crate) async fn remove_plugin(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    blocking(state, move |paths| {
        PluginInstaller::new(paths).remove(&name)?;
        Ok(json!({ "ok": true }))
    })
    .await
}

// ---- trust keyring ----

/// GET /api/trust — trusted author keys.
pub(crate) async fn list_keys(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if let Some(denied) = authorize(&state, &headers, &query) {
        return denied;
    }
    let keys: Vec<_> = Keyring::new(&state.paths)
        .list()
        .into_iter()
        .map(|k| json!({ "name": k.name, "pubkey": k.pubkey }))
        .collect();
    Json(keys).into_response()
}

#[derive(Deserialize)]
pub(crate) struct AddKey {
    name: String,
    pubkey: String,
}

/// POST /api/trust — trust a key `{name, pubkey}`.
pub(crate) async fn add_key(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Json(body): Json<AddKey>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    blocking(state, move |paths| {
        Keyring::new(paths).add(&body.name, &body.pubkey)?;
        Ok(json!({ "ok": true }))
    })
    .await
}

/// DELETE /api/trust/:name — untrust a key.
pub(crate) async fn remove_key(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    if let Some(denied) = authorize_mutation(&state, &headers) {
        return denied;
    }
    blocking(state, move |paths| {
        let removed = Keyring::new(paths).remove(&name)?;
        Ok(json!({ "ok": removed }))
    })
    .await
}
