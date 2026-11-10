//! Host router choice: `[router] name` in `settings.toml`, else the legacy
//! `<riku root>/ROUTER` file that older versions wrote.
//!
//! The router is a host singleton. It lives in a file instead of the
//! `RIKU_ROUTER` env var because git push deploys run through the ssh
//! `command=` key and the post-receive hook, which carry no riku env.
//! `riku init` writes it; every other riku process only reads it.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::{settings, RikuPaths};

/// File name of the host router choice, directly under the riku root.
pub const ROUTER_FILE: &str = "ROUTER";

const NGINX: &str = "nginx";
const CADDY: &str = "caddy";

/// The router that exposes apps on this host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Router {
    /// Built in nginx config generator, the default.
    Nginx,
    /// Built in Caddy site files.
    Caddy,
    /// A router plugin bundle, by name.
    Plugin(String),
}

fn legacy_router(file: &Path) -> Result<Option<Router>> {
    match fs::read_to_string(file) {
        Ok(name) => Ok(Some(Router::from(name.as_str()))),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow!(
            "failed to read router file {}: {e}",
            file.display()
        )),
    }
}

/// The router stored for this host: settings.toml `[router] name`, else the
/// legacy `ROUTER` file; `None` when neither is set. Init uses this to tell a
/// stored choice from the nginx default.
#[allow(clippy::question_mark)]
pub fn stored(paths: &RikuPaths) -> Result<Option<Router>> {
    let settings = match settings::load(paths) {
        Ok(settings) => settings,
        Err(e) => return Err(e),
    };
    match settings.router {
        Some(name) => Ok(Some(Router::from(name.as_str()))),
        None => legacy_router(&paths.router_file),
    }
}

impl Router {
    /// Read the host router: settings.toml first, then the legacy `ROUTER`
    /// file, else nginx. An invalid settings file or an unreadable `ROUTER`
    /// file is an error, so a broken file never silently becomes nginx.
    pub fn load(paths: &RikuPaths) -> Result<Router> {
        match stored(paths) {
            Ok(router) => Ok(router.unwrap_or(Router::Nginx)),
            Err(e) => Err(e),
        }
    }

    /// The name written to the `ROUTER` file.
    pub fn name(&self) -> &str {
        match self {
            Router::Nginx => NGINX,
            Router::Caddy => CADDY,
            Router::Plugin(name) => name,
        }
    }
}

impl From<&str> for Router {
    fn from(raw: &str) -> Self {
        let name = raw.trim().to_ascii_lowercase();
        match name.as_str() {
            "" | NGINX => Router::Nginx,
            CADDY => Router::Caddy,
            _ => Router::Plugin(name),
        }
    }
}

/// Persist the host router choice as `[router] name` in settings.toml.
pub fn save(paths: &RikuPaths, router: &Router) -> Result<()> {
    settings::save_key(paths, "router", "name", Some(router.name()))
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
