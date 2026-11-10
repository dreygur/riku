//! Rendering a site config from a checked route instead of an app's ENV.
//!
//! The router helper runs this as root, so nothing here reads a file, runs a
//! program or fetches anything: every value arrives typed in a [`RouteSite`]
//! and every path the config names arrives in [`SiteFiles`], built by the
//! caller from a checked app name and fixed roots. Values that land unquoted
//! in the config still go through the directive character refusal, as a
//! second line behind the caller's own checks.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::backends::upstream_name;
use crate::sanitize::sanitize_nginx_value;
use crate::static_site::{document_root_in, static_path_dir_in};
use crate::template::{backend_address, register_templates};
use crate::util::backend::Backend;

/// Which template a site takes after `https_only`, which wins over all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteShape {
    /// A proxied app on the plain HTTP port 80.
    Plain,
    /// A proxied app on a mapped port.
    Portmap,
    /// An app served over uWSGI sockets, on a mapped port.
    Wsgi,
    /// The published document root, with no upstream.
    Static,
}

/// Every path the rendered config names, apart from sockets, which travel in
/// the backends.
#[derive(Debug, Clone)]
pub struct SiteFiles {
    pub certificate: PathBuf,
    pub certificate_key: PathBuf,
    pub acme_root: PathBuf,
    pub static_root: PathBuf,
}

/// One site, ready to render.
#[derive(Debug, Clone)]
pub struct RouteSite<'a> {
    pub app: &'a str,
    pub domains: &'a [String],
    pub shape: SiteShape,
    pub https_only: bool,
    /// Also serve HTTPS on 443 with the site's pair, with no redirect from
    /// HTTP. Used for a fallback domain, whose certificate may lapse.
    pub also_tls: bool,
    pub port: u16,
    pub backends: &'a [Backend],
    pub static_locations: &'a [String],
    pub allow_git_folders: bool,
    /// Whether the host has IPv6, so the site also listens on `[::]`.
    pub ipv6: bool,
    pub files: &'a SiteFiles,
}

fn template_name(site: &RouteSite) -> &'static str {
    if site.https_only {
        return "nginx_https_only.conf.tera";
    }
    match site.shape {
        SiteShape::Wsgi => "nginx_wsgi.conf.tera",
        SiteShape::Portmap => "nginx_portmap.conf.tera",
        SiteShape::Static => "nginx_static.conf.tera",
        SiteShape::Plain => "nginx.conf.tera",
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn insert_static(context: &mut tera::Context, site: &RouteSite) -> Result<()> {
    let mut aliases: Vec<HashMap<&str, String>> = Vec::new();
    for (index, location) in site.static_locations.iter().enumerate() {
        let location = match sanitize_nginx_value("static location", location) {
            Ok(location) => location,
            Err(e) => return Err(anyhow!("static path {index}: {e}")),
        };
        let alias = static_path_dir_in(&site.files.static_root, site.app, index);
        aliases.push(HashMap::from([
            ("location", location),
            ("alias", text(&alias)),
        ]));
    }
    context.insert("RIKU_STATIC_ALIASES", &aliases);
    context.insert(
        "RIKU_NGINX_DOCUMENT_ROOT",
        &text(&document_root_in(&site.files.static_root, site.app)),
    );
    Ok(())
}

fn build_context(site: &RouteSite) -> Result<tera::Context> {
    let mut context = tera::Context::new();
    let domains = match sanitize_nginx_value("domains", &site.domains.join(" ")) {
        Ok(domains) => domains,
        Err(e) => return Err(anyhow!("domains: {e}")),
    };
    let addresses: Result<Vec<String>> = site.backends.iter().map(backend_address).collect();
    let addresses = match addresses {
        Ok(addresses) => addresses,
        Err(e) => return Err(anyhow!("refusing backend: {e}")),
    };

    context.insert("IPV6", &site.ipv6);
    context.insert("ALSO_TLS", &(site.also_tls && !site.https_only));
    context.insert("APP", site.app);
    context.insert("APP_UPSTREAM", &upstream_name(site.app));
    context.insert("RIKU_DOMAINS", &domains);
    context.insert("RIKU_BACKENDS", &addresses);
    context.insert("RIKU_NGINX_EXTERNAL_PORT", &site.port.to_string());
    context.insert(
        "RIKU_NGINX_ALLOW_GIT_FOLDERS",
        &site.allow_git_folders.to_string(),
    );
    context.insert("SSL_CERTIFICATE", &text(&site.files.certificate));
    context.insert("SSL_CERTIFICATE_KEY", &text(&site.files.certificate_key));
    context.insert("ACME_WWW", &text(&site.files.acme_root));
    if site.shape == SiteShape::Wsgi {
        context.insert("RIKU_NGINX_WSGI", "true");
    }
    if let Err(e) = insert_static(&mut context, site) {
        return Err(anyhow!("static files: {e}"));
    }
    Ok(context)
}

/// The config text for `site`.
pub fn render_route_site(site: &RouteSite) -> Result<String> {
    let mut tera = tera::Tera::default();
    if let Err(e) = register_templates(&mut tera) {
        return Err(anyhow!("cannot load nginx templates: {e}"));
    }
    let context = match build_context(site) {
        Ok(context) => context,
        Err(e) => return Err(anyhow!("cannot build nginx context: {e}")),
    };
    let name = template_name(site);
    match tera.render(name, &context) {
        Ok(text) => Ok(text),
        Err(e) => Err(anyhow!("failed to render {name} for '{}': {e}", site.app)),
    }
}

#[cfg(test)]
#[path = "route_site_tests.rs"]
mod tests;
