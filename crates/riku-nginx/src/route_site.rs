//! Rendering a site config from a checked route instead of an app's ENV.
//!
//! The router helper runs this as root, so nothing here reads a file, runs a
//! program or fetches anything: every value arrives typed in a [`RouteSite`]
//! and every path the config names arrives in [`SiteFiles`], built by the
//! caller from a checked app name and fixed roots. Values that land unquoted
//! in the config still go through the directive character refusal that the
//! ENV path applies, as a second line behind the caller's own checks.
//!
//! The output is the same template text the ENV path renders, so the two
//! cannot drift apart.

use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::app_snippet::snippet_directives;
use crate::backends::upstream_name;
use crate::context_cache::CACHE_KEYS_ZONE;
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

/// The proxy cache of a site. `size_gb` bounds the disk it may fill.
#[derive(Debug, Clone, Copy)]
pub struct SiteCache<'a> {
    pub prefixes: &'a [String],
    pub time_s: u32,
    pub size_gb: u32,
    pub expiry_s: u32,
    pub redirects_s: u32,
    pub any_s: u32,
    pub control_s: u32,
}

/// Every path the rendered config names, apart from sockets, which travel in
/// the backends.
#[derive(Debug, Clone)]
pub struct SiteFiles {
    pub certificate: PathBuf,
    pub certificate_key: PathBuf,
    pub cloudflare_conf: PathBuf,
    pub cache_dir: PathBuf,
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
    pub ipv4: Ipv4Addr,
    pub ipv6: Option<Ipv6Addr>,
    pub port: u16,
    pub backends: &'a [Backend],
    pub static_locations: &'a [String],
    pub cache: Option<SiteCache<'a>>,
    pub cloudflare_acl: bool,
    pub allow_git_folders: bool,
    pub catch_all: Option<&'a str>,
    pub csp: Option<&'a str>,
    pub snippet: &'a str,
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

fn insert_cache(context: &mut tera::Context, site: &RouteSite, cache: SiteCache) -> Result<()> {
    let mut prefixes = Vec::new();
    for prefix in cache.prefixes {
        match sanitize_nginx_value("cache prefix", prefix) {
            Ok(prefix) => prefixes.push(prefix),
            Err(e) => return Err(anyhow!("cache: {e}")),
        }
    }
    context.insert("RIKU_NGINX_CACHE_PREFIXES", &prefixes.join(","));
    context.insert("RIKU_NGINX_CACHE_PATH", &text(&site.files.cache_dir));
    context.insert("RIKU_NGINX_CACHE_ZONE", CACHE_KEYS_ZONE);
    context.insert("RIKU_NGINX_CACHE_SIZE", &cache.size_gb.to_string());
    context.insert("RIKU_NGINX_CACHE_TIME", &cache.time_s.to_string());
    context.insert("RIKU_NGINX_CACHE_EXPIRY", &cache.expiry_s.to_string());
    context.insert("RIKU_NGINX_CACHE_REDIRECTS", &cache.redirects_s.to_string());
    context.insert("RIKU_NGINX_CACHE_ANY", &cache.any_s.to_string());
    context.insert("RIKU_NGINX_CACHE_CONTROL", &cache.control_s.to_string());
    Ok(())
}

fn insert_optional_text(context: &mut tera::Context, site: &RouteSite) -> Result<()> {
    if let Some(name) = site.catch_all {
        match sanitize_nginx_value("catch_all", name) {
            Ok(name) => context.insert("RIKU_NGINX_CATCH_ALL", &name),
            Err(e) => return Err(anyhow!("catch_all: {e}")),
        }
    }
    if let Some(policy) = site.csp {
        context.insert("RIKU_NGINX_CSP", policy);
    }
    Ok(())
}

fn build_context(site: &RouteSite) -> Result<tera::Context> {
    let mut context = tera::Context::new();
    let domains = match sanitize_nginx_value("domains", &site.domains.join(" ")) {
        Ok(domains) => domains,
        Err(e) => return Err(anyhow!("domains: {e}")),
    };
    let directives = match snippet_directives(site.snippet) {
        Ok(directives) => directives,
        Err(e) => return Err(anyhow!("refusing snippet: {e}")),
    };
    let addresses: Result<Vec<String>> = site.backends.iter().map(backend_address).collect();
    let addresses = match addresses {
        Ok(addresses) => addresses,
        Err(e) => return Err(anyhow!("refusing backend: {e}")),
    };

    context.insert("APP", site.app);
    context.insert("APP_UPSTREAM", &upstream_name(site.app));
    context.insert("RIKU_DOMAINS", &domains);
    context.insert("RIKU_BACKENDS", &addresses);
    context.insert("RIKU_NGINX_IPV4_ADDRESS", &site.ipv4.to_string());
    let ipv6 = site.ipv6.map(|address| format!("[{address}]"));
    context.insert("RIKU_NGINX_IPV6_ADDRESS", &ipv6.unwrap_or_default());
    context.insert("RIKU_NGINX_EXTERNAL_PORT", &site.port.to_string());
    context.insert(
        "RIKU_NGINX_CLOUDFLARE_ACL",
        &site.cloudflare_acl.to_string(),
    );
    context.insert(
        "RIKU_NGINX_ALLOW_GIT_FOLDERS",
        &site.allow_git_folders.to_string(),
    );
    context.insert("RIKU_NGINX_INCLUDE_CONTENT", site.snippet);
    context.insert("RIKU_NGINX_SNIPPET_DIRECTIVES", &directives);
    context.insert("SSL_CERTIFICATE", &text(&site.files.certificate));
    context.insert("SSL_CERTIFICATE_KEY", &text(&site.files.certificate_key));
    context.insert("CLOUDFLARE_CONF", &text(&site.files.cloudflare_conf));
    context.insert("ACME_WWW", &text(&site.files.acme_root));
    if site.shape == SiteShape::Wsgi {
        context.insert("RIKU_NGINX_WSGI", "true");
    }
    if let Err(e) = insert_static(&mut context, site) {
        return Err(anyhow!("static files: {e}"));
    }
    if let (Some(cache), false) = (site.cache, site.shape == SiteShape::Wsgi) {
        if let Err(e) = insert_cache(&mut context, site, cache) {
            return Err(anyhow!("proxy cache: {e}"));
        }
    }
    if let Err(e) = insert_optional_text(&mut context, site) {
        return Err(anyhow!("site text: {e}"));
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
