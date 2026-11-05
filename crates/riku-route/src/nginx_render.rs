//! From a checked route to nginx config text.
//!
//! Every path the text names is built here from the checked app name and the
//! [`NginxRoots`]: unix backends become `<socket root>/<app>@<n>.sock` or
//! `<socket root>/<app>.sock`, the certificate pair lives in the app's
//! directory under the cert root, the cache in the app's directory under the
//! cache root. The route only supplies copy numbers and text the gate checked.

use anyhow::Result;
use riku_container::types::AppName;
use riku_nginx::{render_route_site, RouteSite, SiteCache, SiteFiles, SiteShape};
use riku_util::backend::Backend;
use riku_util::file_names::copy_socket;
use riku_util::route_plan::HttpsPolicy;

use crate::nginx_roots::NginxRoots;
use crate::schema::{Route, RouteBackend, SiteKind};

pub(crate) const CERTIFICATE_FILE: &str = "fullchain.pem";
pub(crate) const KEY_FILE: &str = "key.pem";

fn shape(kind: SiteKind) -> SiteShape {
    match kind {
        SiteKind::Plain => SiteShape::Plain,
        SiteKind::Portmap => SiteShape::Portmap,
        SiteKind::Wsgi => SiteShape::Wsgi,
        SiteKind::Static => SiteShape::Static,
    }
}

fn backend(roots: &NginxRoots, app: &AppName, backend: &RouteBackend) -> Backend {
    match backend {
        RouteBackend::Tcp { host, port } => Backend::Tcp {
            host: *host,
            port: *port,
        },
        RouteBackend::UnixCopy(copy) => Backend::Unix(
            roots
                .socket_root
                .join(copy_socket::file_name(app.as_str(), *copy)),
        ),
        RouteBackend::UnixDefault => Backend::Unix(roots.socket_root.join(format!("{app}.sock"))),
    }
}

fn files(roots: &NginxRoots, app: &AppName) -> SiteFiles {
    let cert_dir = roots.cert_dir(app);
    SiteFiles {
        certificate: cert_dir.join(CERTIFICATE_FILE),
        certificate_key: cert_dir.join(KEY_FILE),
        cloudflare_conf: roots.cloudflare_conf.clone(),
        cache_dir: roots.cache_dir(app),
        acme_root: roots.acme_root.clone(),
        static_root: roots.static_root.clone(),
    }
}

/// The config text for `route`, which must have passed the gate.
pub(crate) fn render(roots: &NginxRoots, app: &AppName, route: &Route) -> Result<String> {
    let backends: Vec<Backend> = route
        .backends
        .iter()
        .map(|each| backend(roots, app, each))
        .collect();
    let files = files(roots, app);
    let site = RouteSite {
        app: app.as_str(),
        domains: &route.domains,
        shape: shape(route.kind),
        https_only: route.https == HttpsPolicy::Only,
        ipv4: route.listen.bind,
        ipv6: route.listen.ipv6,
        port: route.listen.port,
        backends: &backends,
        static_locations: &route.static_paths,
        cache: route.cache.as_ref().map(|cache| SiteCache {
            prefixes: &cache.prefixes,
            time_s: cache.time_s,
            size_gb: cache.size_gb,
            expiry_s: cache.expiry_s,
            redirects_s: cache.redirects_s,
            any_s: cache.any_s,
            control_s: cache.control_s,
        }),
        cloudflare_acl: route.cloudflare_acl,
        allow_git_folders: route.allow_git_folders,
        catch_all: route.catch_all.as_deref(),
        csp: route.csp.as_deref(),
        snippet: route.snippet.as_deref().unwrap_or_default(),
        files: &files,
    };
    render_route_site(&site)
}
