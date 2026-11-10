//! The shared path mode site: one server on the shared port with a
//! `location /<app>/` per path mode app, each proxying to that app's own
//! upstream with the prefix stripped and sent as `X-Forwarded-Prefix`.
//!
//! Security: runs as root in the router helper, so it reads no file and
//! runs no program. App names have passed `AppName` in the helper and are
//! checked again here for nginx directive characters; backend addresses go
//! through the same refusal as a single app site.

use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::backends::upstream_name;
use crate::sanitize::sanitize_nginx_value;
use crate::template::{backend_address, register_templates};
use crate::util::backend::Backend;

const TEMPLATE: &str = "nginx_paths.conf.tera";
const UPSTREAM_SUFFIX: &str = "_path";

/// One path mode app and the copies that serve it.
#[derive(Debug, Clone)]
pub struct PathApp {
    pub app: String,
    pub backends: Vec<Backend>,
}

/// The shared site, ready to render.
#[derive(Debug, Clone)]
pub struct PathsSite<'a> {
    /// The shared port, `[routing] shared_port`.
    pub port: u16,
    /// Whether the host has IPv6, so the site also listens on `[::]`.
    pub ipv6: bool,
    pub apps: &'a [PathApp],
}

fn app_context(entry: &PathApp) -> Result<HashMap<&'static str, tera::Value>> {
    let name = match sanitize_nginx_value("app", &entry.app) {
        Ok(name) => name,
        Err(e) => return Err(anyhow!("app {:?}: {e}", entry.app)),
    };
    let addresses: Result<Vec<String>> = entry.backends.iter().map(backend_address).collect();
    let addresses = match addresses {
        Ok(addresses) => addresses,
        Err(e) => return Err(anyhow!("refusing a backend of '{name}': {e}")),
    };
    let upstream = format!("{}{UPSTREAM_SUFFIX}", upstream_name(&name));
    Ok(HashMap::from([
        ("name", tera::Value::from(name)),
        ("upstream", tera::Value::from(upstream)),
        ("addresses", tera::Value::from(addresses)),
    ]))
}

/// The config text for the shared path site.
#[allow(clippy::question_mark)]
pub fn render_paths_site(site: &PathsSite) -> Result<String> {
    let apps: Result<Vec<_>> = site.apps.iter().map(app_context).collect();
    let apps = match apps {
        Ok(apps) => apps,
        Err(e) => return Err(e),
    };
    let mut tera = tera::Tera::default();
    if let Err(e) = register_templates(&mut tera) {
        return Err(anyhow!("cannot load nginx templates: {e}"));
    }
    let mut context = tera::Context::new();
    context.insert("PORT", &site.port);
    context.insert("IPV6", &site.ipv6);
    context.insert("APPS", &apps);
    tera.render(TEMPLATE, &context)
        .map_err(|e| anyhow!("cannot render the path site: {e}"))
}

#[cfg(test)]
#[path = "paths_site_tests.rs"]
mod tests;
