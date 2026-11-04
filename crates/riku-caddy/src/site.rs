//! Site file rendering and storage.
//!
//! Every value spliced into a site file is typed: domains are validated
//! `Domain`s, backends are typed [`Backend`]s, and the app name was validated
//! before deploy reached this point. No raw ENV string is written.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use riku_config::RikuPaths;
use riku_util::backend::Backend;
use riku_util::route_plan::{HttpsPolicy, RoutePlan};
use riku_util::write_atomic;

const SITE_EXTENSION: &str = "caddy";
const PLAIN_HTTP_SCHEME: &str = "http://";

/// Where `app`'s site file lives: `<riku root>/caddy/<app>.caddy`.
pub fn site_path(app: &str, paths: &RikuPaths) -> PathBuf {
    paths.caddy_root.join(format!("{app}.{SITE_EXTENSION}"))
}

fn site_addresses(plan: &RoutePlan) -> Vec<String> {
    let bare = plan.domains.iter().map(|d| d.to_string());
    match plan.https {
        HttpsPolicy::Only => bare.collect(),
        HttpsPolicy::AlsoPlainHttp => {
            let plain = plan
                .domains
                .iter()
                .map(|d| format!("{PLAIN_HTTP_SCHEME}{d}"));
            bare.chain(plain).collect()
        }
    }
}

fn upstreams(plan: &RoutePlan) -> Result<String> {
    if plan.backends.is_empty() {
        return Err(anyhow!("the route plan has no backend to proxy to"));
    }
    if plan.backends.iter().any(|b| matches!(b, Backend::Unix(_))) {
        return Err(anyhow!("Caddy proxies TCP only, not unix sockets"));
    }
    let addresses: Vec<String> = plan.backends.iter().map(Backend::to_string).collect();
    Ok(addresses.join(" "))
}

/// The site file text for `app`. Every backend is an upstream on one
/// `reverse_proxy` line. Fails when the plan has no backend or a unix socket.
#[allow(clippy::question_mark)]
pub fn render_site(app: &str, plan: &RoutePlan) -> Result<String> {
    let upstreams = match upstreams(plan) {
        Ok(upstreams) => upstreams,
        Err(e) => return Err(e),
    };
    Ok(format!(
        "# riku app: {app}\n{addresses} {{\n\treverse_proxy {upstreams}\n}}\n",
        app = app,
        addresses = site_addresses(plan).join(", "),
    ))
}

/// Write `app`'s site file atomically, so Caddy never reads half of one.
#[allow(clippy::question_mark)]
pub fn write_site(app: &str, plan: &RoutePlan, paths: &RikuPaths) -> Result<()> {
    let text = match render_site(app, plan) {
        Ok(text) => text,
        Err(e) => return Err(e),
    };
    if let Err(e) = fs::create_dir_all(&paths.caddy_root) {
        return Err(anyhow!(
            "failed to create {}: {e}",
            paths.caddy_root.display()
        ));
    }
    write_atomic(&site_path(app, paths), text.as_bytes())
}

/// Delete `app`'s site file. A missing file is not an error.
pub fn remove_site(app: &str, paths: &RikuPaths) -> Result<()> {
    let path = site_path(app, paths);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow!("failed to remove {}: {e}", path.display())),
    }
}

#[cfg(test)]
#[path = "site_tests.rs"]
mod tests;
