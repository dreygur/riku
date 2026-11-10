//! Site file rendering and storage.
//!
//! Every value spliced into a site file is typed: domains are validated
//! `Domain`s, backends are typed [`Backend`]s, and the app name was validated
//! before deploy reached this point. No raw ENV string is written.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use riku_config::RikuPaths;
use riku_util::backend::Backend;
use riku_util::route_plan::{HttpsPolicy, RoutePlan};

const SITE_EXTENSION: &str = "caddy";
const PLAIN_HTTP_SCHEME: &str = "http://";

/// Where `app`'s site file lives: `<riku root>/caddy/<app>.caddy`.
pub fn site_path(app: &str, paths: &RikuPaths) -> PathBuf {
    paths.caddy_root.join(format!("{app}.{SITE_EXTENSION}"))
}

fn site_addresses(plan: &RoutePlan) -> Vec<String> {
    let bare = plan.domains.iter().map(|d| d.to_string());
    let mut addresses: Vec<String> = match plan.https {
        HttpsPolicy::Only => bare.collect(),
        HttpsPolicy::AlsoPlainHttp => {
            let plain = plan
                .domains
                .iter()
                .map(|d| format!("{PLAIN_HTTP_SCHEME}{d}"));
            bare.chain(plain).collect()
        }
    };
    if let Some(port) = plan.open_port {
        addresses.push(format!("{PLAIN_HTTP_SCHEME}:{port}"));
    }
    addresses
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

/// Where Caddy reads an app's own certificate and key. The caller builds
/// both paths; the renderer only quotes them into the site.
#[derive(Debug, Clone, PartialEq)]
pub struct TlsFiles {
    pub certificate: PathBuf,
    pub key: PathBuf,
}

fn quoted(what: &str, path: &Path) -> Result<String> {
    let text = path.to_string_lossy();
    if text
        .chars()
        .any(|c| c.is_control() || c == '"' || c == '\\')
    {
        return Err(anyhow!(
            "the {what} path {text:?} cannot be written into a site file"
        ));
    }
    Ok(format!("\"{text}\""))
}

#[allow(clippy::question_mark)]
fn tls_line(tls: &TlsFiles) -> Result<String> {
    let certificate = match quoted("certificate", &tls.certificate) {
        Ok(path) => path,
        Err(e) => return Err(e),
    };
    let key = match quoted("key", &tls.key) {
        Ok(path) => path,
        Err(e) => return Err(e),
    };
    Ok(format!("\ttls {certificate} {key}\n"))
}

/// The site file text for `app`. Every backend is an upstream on one
/// `reverse_proxy` line. Fails when the plan has no backend or a unix socket.
pub fn render_site(app: &str, plan: &RoutePlan) -> Result<String> {
    render_site_with_tls(app, plan, None)
}

fn block(addresses: &[String], tls_line: &str, upstreams: &str) -> String {
    format!(
        "{} {{\n{tls_line}\treverse_proxy {upstreams}\n}}\n",
        addresses.join(", ")
    )
}

/// [`render_site`] with a `tls <certificate> <key>` line when the app serves
/// its own certificate, so Caddy does not issue one for the site. Caddy
/// refuses that line in a block that also listens on plain HTTP, so the
/// `http://` addresses then get a block of their own.
#[allow(clippy::question_mark)]
pub fn render_site_with_tls(app: &str, plan: &RoutePlan, tls: Option<&TlsFiles>) -> Result<String> {
    let upstreams = match upstreams(plan) {
        Ok(upstreams) => upstreams,
        Err(e) => return Err(e),
    };
    let addresses = site_addresses(plan);
    let Some(tls) = tls else {
        return Ok(format!(
            "# riku app: {app}\n{}",
            block(&addresses, "", &upstreams)
        ));
    };
    let tls_line = match tls_line(tls) {
        Ok(line) => line,
        Err(e) => return Err(e),
    };
    let (plain, secure): (Vec<String>, Vec<String>) = addresses
        .into_iter()
        .partition(|address| address.starts_with(PLAIN_HTTP_SCHEME));
    let mut site = format!("# riku app: {app}\n");
    if !secure.is_empty() {
        site.push_str(&block(&secure, &tls_line, &upstreams));
    }
    if !plain.is_empty() {
        site.push_str(&block(&plain, "", &upstreams));
    }
    Ok(site)
}

#[cfg(test)]
#[path = "site_tests.rs"]
mod tests;
