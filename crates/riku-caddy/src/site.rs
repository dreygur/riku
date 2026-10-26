//! Site file rendering and storage.
//!
//! Every value spliced into a site file is typed: domains are validated
//! [`Domain`]s, the port is a `u16`, and the app name was validated before
//! deploy reached this point. No raw ENV string is written.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use riku_config::RikuPaths;
use riku_util::domain::Domain;
use riku_util::write_atomic;

const SITE_EXTENSION: &str = "caddy";
const UPSTREAM_HOST: &str = "127.0.0.1";
const PLAIN_HTTP_SCHEME: &str = "http://";

/// How a site answers plain HTTP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpsPolicy {
    /// Caddy's automatic HTTP to HTTPS redirect applies.
    Only,
    /// Plain HTTP is served too, through explicit `http://` addresses.
    AlsoPlainHttp,
}

/// What a site file routes: the app's domains to its local upstream port.
pub struct SiteRequest {
    pub domains: Vec<Domain>,
    pub upstream_port: u16,
    pub https: HttpsPolicy,
}

/// Where `app`'s site file lives: `<riku root>/caddy/<app>.caddy`.
pub fn site_path(app: &str, paths: &RikuPaths) -> PathBuf {
    paths.caddy_root.join(format!("{app}.{SITE_EXTENSION}"))
}

fn site_addresses(site: &SiteRequest) -> Vec<String> {
    let bare = site.domains.iter().map(|d| d.to_string());
    match site.https {
        HttpsPolicy::Only => bare.collect(),
        HttpsPolicy::AlsoPlainHttp => {
            let plain = site
                .domains
                .iter()
                .map(|d| format!("{PLAIN_HTTP_SCHEME}{d}"));
            bare.chain(plain).collect()
        }
    }
}

/// The site file text for `app`.
pub fn render_site(app: &str, site: &SiteRequest) -> String {
    format!(
        "# riku app: {app}\n{addresses} {{\n\treverse_proxy {UPSTREAM_HOST}:{port}\n}}\n",
        addresses = site_addresses(site).join(", "),
        port = site.upstream_port,
    )
}

/// Write `app`'s site file atomically, so Caddy never reads half of one.
pub fn write_site(app: &str, site: &SiteRequest, paths: &RikuPaths) -> Result<()> {
    if let Err(e) = fs::create_dir_all(&paths.caddy_root) {
        return Err(anyhow!(
            "failed to create {}: {e}",
            paths.caddy_root.display()
        ));
    }
    write_atomic(&site_path(app, paths), render_site(app, site).as_bytes())
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
