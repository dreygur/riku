//! The messages the deploy side and the router helper exchange.
//!
//! One request is one JSON object, one reply is one JSON object. A request
//! carries `"v": 1` and an `"op"`; unknown fields are refused at every level
//! (the route structs refuse them too), and a version other than
//! [`PROTOCOL_VERSION`] is refused rather than guessed at. Nothing in a
//! request is a path: the app is a name that [`AppName`](riku_container::types::AppName)
//! checks before the helper uses it.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::schema::{Listen, PathApp, Route};

/// The only protocol version this build speaks.
pub const PROTOCOL_VERSION: u8 = 1;

/// The router a request is for. A request that names none is for nginx.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouterKind {
    #[default]
    Nginx,
    Caddy,
}

/// What the deploy side asks the helper to do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// Install or replace the router config of `app` and reload the router.
    Apply {
        app: String,
        #[serde(default)]
        router: RouterKind,
        route: Box<Route>,
    },
    /// Drop the router config of `app` and reload the router.
    Remove {
        app: String,
        #[serde(default)]
        router: RouterKind,
    },
    /// Replace the shared path mode site with one location per app in
    /// `apps`, listening on `listen`, and reload the router. An empty list
    /// removes the site.
    ApplyPaths {
        #[serde(default)]
        router: RouterKind,
        listen: Listen,
        apps: Vec<PathApp>,
    },
    /// Ask whether the helper is alive and which build it is.
    Status {},
}

/// The step that refused a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// The request did not parse, was too large or failed the route gate.
    Schema,
    /// The config could not be rendered or installed.
    Render,
    /// The router's own syntax check refused the config.
    Check,
    /// The router did not accept the reload.
    Reload,
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Stage::Schema => "schema",
            Stage::Render => "render",
            Stage::Check => "check",
            Stage::Reload => "reload",
        };
        f.write_str(name)
    }
}

/// What the helper answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reply", rename_all = "snake_case")]
pub enum Reply {
    /// The request was carried out. `reloaded` says whether the router
    /// reloaded; a `remove` of an app with no config does not reload.
    Ok { reloaded: bool },
    /// The request was refused at `stage`; `error` is the reason.
    Failed { stage: Stage, error: String },
    /// The answer to `status`: the helper's version.
    Status { version: String },
}

/// Parse one request. The caller has already bounded `bytes`; this refuses
/// anything that is not exactly one known request of the current version.
///
/// The version is taken off the object before the rest is read as a
/// [`Request`], because serde cannot refuse unknown fields on a flattened
/// enum.
pub fn parse_request(bytes: &[u8]) -> Result<Request> {
    let mut object: serde_json::Map<String, serde_json::Value> = match serde_json::from_slice(bytes)
    {
        Ok(object) => object,
        Err(e) => return Err(anyhow!("malformed request: {e}")),
    };
    let version = object.remove("v").and_then(|v| v.as_u64());
    if version != Some(u64::from(PROTOCOL_VERSION)) {
        return Err(anyhow!(
            "protocol version {version:?} is not supported, this helper speaks {PROTOCOL_VERSION}"
        ));
    }
    match serde_json::from_value(serde_json::Value::Object(object)) {
        Ok(request) => Ok(request),
        Err(e) => Err(anyhow!("malformed request: {e}")),
    }
}

/// The bytes the client sends for `request`.
pub fn encode_request(request: &Request) -> Result<Vec<u8>> {
    let mut object = match serde_json::to_value(request) {
        Ok(serde_json::Value::Object(object)) => object,
        Ok(_) => return Err(anyhow!("a request must encode as an object")),
        Err(e) => return Err(anyhow!("failed to encode the request: {e}")),
    };
    object.insert("v".to_string(), PROTOCOL_VERSION.into());
    match serde_json::to_vec(&object) {
        Ok(bytes) => Ok(bytes),
        Err(e) => Err(anyhow!("failed to encode the request: {e}")),
    }
}

/// The bytes the helper sends for `reply`.
pub fn encode_reply(reply: &Reply) -> Result<Vec<u8>> {
    match serde_json::to_vec(reply) {
        Ok(bytes) => Ok(bytes),
        Err(e) => Err(anyhow!("failed to encode the reply: {e}")),
    }
}

/// Parse a reply the helper sent.
pub fn parse_reply(bytes: &[u8]) -> Result<Reply> {
    match serde_json::from_slice(bytes) {
        Ok(reply) => Ok(reply),
        Err(e) => Err(anyhow!("malformed reply from the router helper: {e}")),
    }
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
