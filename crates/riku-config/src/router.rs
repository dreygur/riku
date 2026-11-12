//! Host router choice: `[router] name` in `settings.toml`.
//!
//! The router is a host singleton. It lives in a file instead of the
//! `RIKU_ROUTER` env var because git push deploys run through the ssh
//! `command=` key and the post-receive hook, which carry no riku env.
//! `riku init` writes it; every other riku process only reads it.

use anyhow::Result;

use crate::{settings, RikuPaths};

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

/// The router stored for this host in settings.toml `[router] name`, or
/// `None` when it is not set. Init uses this to tell a stored choice from the
/// nginx default.
pub fn stored(paths: &RikuPaths) -> Result<Option<Router>> {
    match settings::load(paths) {
        Ok(settings) => Ok(settings.router.map(|name| Router::from(name.as_str()))),
        Err(e) => Err(e),
    }
}

impl Router {
    /// Read the host router from settings.toml, else nginx. An invalid
    /// settings file is an error, so a broken file never silently becomes
    /// nginx.
    pub fn load(paths: &RikuPaths) -> Result<Router> {
        match stored(paths) {
            Ok(router) => Ok(router.unwrap_or(Router::Nginx)),
            Err(e) => Err(e),
        }
    }

    /// The name stored in settings.toml.
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
