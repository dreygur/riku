//! Host router choice, stored in `<riku root>/ROUTER`.
//!
//! The router is a host singleton. It lives in a file instead of the
//! `RIKU_ROUTER` env var because git push deploys run through the ssh
//! `command=` key and the post-receive hook, which carry no riku env.
//! `riku init` writes the file; every other riku process only reads it.

use std::fs;
use std::io::ErrorKind;

use anyhow::{anyhow, Result};

use crate::RikuPaths;

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

impl Router {
    /// Read the host router. A missing file means nginx; any other read
    /// failure is an error, so a broken file never silently becomes nginx.
    pub fn load(paths: &RikuPaths) -> Result<Router> {
        match fs::read_to_string(&paths.router_file) {
            Ok(name) => Ok(Router::from(name.as_str())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Router::Nginx),
            Err(e) => Err(anyhow!(
                "failed to read router file {}: {e}",
                paths.router_file.display()
            )),
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

/// Persist the host router choice.
pub fn save(paths: &RikuPaths, router: &Router) -> Result<()> {
    match fs::write(&paths.router_file, format!("{}\n", router.name())) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!(
            "failed to write router file {}: {e}",
            paths.router_file.display()
        )),
    }
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
