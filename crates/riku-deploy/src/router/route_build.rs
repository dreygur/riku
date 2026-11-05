//! Building the route the router helper accepts from an app's ENV, its
//! running copies and the certificates it already has.
//!
//! Security: the route is built from the app's own ENV, which an app repo
//! can set, so this module only parses; the `riku_route` gate runs on the
//! result for an early error and again in the helper. No ENV value becomes a
//! path: a key that names one is refused by name, and a unix backend is only
//! accepted as one of the app's own sockets, then sent as a copy number or
//! as the default socket.
//!
//! A Caddy route carries only what Caddy uses (domains, https policy, TCP
//! backends); the nginx only fields stay unset, so an app that deploys on
//! Caddy today keeps deploying.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;

use anyhow::{anyhow, Result};
use riku_container::types::AppName;
use riku_route::{check_caddy_route, Listen, Route, RouteBackend, SiteKind};

use super::route_env::{cache, listen, tls};
use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::backend::Backend;
use crate::util::domain::{parse_domains, Domain};
use crate::util::file_names::copy_socket;
use crate::util::flag::{https_only, is_on};
use crate::util::route_plan::HttpsPolicy;

pub(super) const CATCH_ALL_NAME: &str = "_";
const REFUSED_KEYS: [(&str, &str); 2] = [
    ("RIKU_NGINX_CACHE_PATH", "the helper picks the cache path"),
    ("RIKU_NGINX_SOCKET", "the helper picks socket paths"),
];

fn route_error(app: &AppName, cause: anyhow::Error) -> anyhow::Error {
    anyhow!("cannot build the route of '{app}': {cause}")
}

fn refused_key(env: &HashMap<String, String>) -> Option<&'static (&'static str, &'static str)> {
    REFUSED_KEYS.iter().find(|(key, _)| env.contains_key(*key))
}

/// What nginx's generator would serve: the listed names, `<app>.example.com`
/// when `RIKU_DOMAINS` is unset, and `_` as the catch all name.
pub(super) fn nginx_domains(app: &AppName, env: &HashMap<String, String>) -> Result<Vec<String>> {
    let Some(raw) = env.get("RIKU_DOMAINS") else {
        return Ok(vec![format!("{app}.example.com")]);
    };
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            if entry == CATCH_ALL_NAME {
                return Ok(entry.to_string());
            }
            match Domain::try_from(entry) {
                Ok(domain) => Ok(domain.to_string()),
                Err(e) => Err(anyhow!("invalid RIKU_DOMAINS: {e}")),
            }
        })
        .collect()
}

fn caddy_domains(env: &HashMap<String, String>) -> Result<Vec<String>> {
    let raw = env.get("RIKU_DOMAINS").map_or("", String::as_str);
    match parse_domains(raw) {
        Ok(domains) => Ok(domains.iter().map(ToString::to_string).collect()),
        Err(e) => Err(anyhow!("invalid RIKU_DOMAINS: {e}")),
    }
}

/// The template nginx picks after `https: only`, in `select_template` order.
/// Caddy ignores a port mapping, so it never gets `Portmap`.
fn site_kind(env: &HashMap<String, String>, router: &Router) -> SiteKind {
    if env.contains_key("RIKU_NGINX_WSGI") {
        SiteKind::Wsgi
    } else if env.contains_key("RIKU_NGINX_PORTMAP") && !matches!(router, Router::Caddy) {
        SiteKind::Portmap
    } else if env.contains_key("RIKU_NGINX_STATIC") {
        SiteKind::Static
    } else {
        SiteKind::Plain
    }
}

fn socket_backend(app: &AppName, socket: &Path, paths: &RikuPaths) -> Result<RouteBackend> {
    let name = socket.file_name().and_then(|name| name.to_str());
    if socket.parent() == Some(paths.socket_root.as_path()) {
        if name == Some(format!("{app}.sock").as_str()) {
            return Ok(RouteBackend::UnixDefault);
        }
        if let Some((owner, copy)) = name.and_then(copy_socket::parse) {
            if owner == app.as_str() {
                return Ok(RouteBackend::UnixCopy(copy));
            }
        }
    }
    Err(anyhow!(
        "socket {} is not a socket of '{app}' in {}; the helper builds socket paths itself",
        socket.display(),
        paths.socket_root.display()
    ))
}

/// A router on this host reaches a TCP backend over loopback, so the
/// all interfaces address an app may bind to is sent as loopback.
fn route_backend(app: &AppName, backend: &Backend, paths: &RikuPaths) -> Result<RouteBackend> {
    match backend {
        Backend::Tcp { host, port } => {
            let host = match host {
                IpAddr::V4(v4) if v4.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
                other => *other,
            };
            Ok(RouteBackend::Tcp { host, port: *port })
        }
        Backend::Unix(socket) => socket_backend(app, socket, paths),
    }
}

fn route_backends(
    app: &AppName,
    backends: &[Backend],
    paths: &RikuPaths,
) -> Result<Vec<RouteBackend>> {
    backends
        .iter()
        .map(|backend| route_backend(app, backend, paths))
        .collect()
}

fn text_key(env: &HashMap<String, String>, key: &str) -> Option<String> {
    env.get(key).filter(|value| !value.is_empty()).cloned()
}

fn flag(env: &HashMap<String, String>, key: &str) -> bool {
    env.get(key).is_some_and(|value| is_on(value))
}

/// A route with only what Caddy uses.
fn caddy_route(
    domains: Vec<String>,
    https: HttpsPolicy,
    backends: Vec<RouteBackend>,
    kind: SiteKind,
) -> Route {
    Route {
        domains,
        https,
        backends,
        listen: Listen::default(),
        tls: None,
        static_paths: Vec::new(),
        kind,
        cache: None,
        cloudflare_acl: false,
        allow_git_folders: false,
        catch_all: None,
        csp: None,
        snippet: None,
    }
}

fn nginx_route(
    app: &AppName,
    app_path: &Path,
    env: &HashMap<String, String>,
    base: Route,
    paths: &RikuPaths,
) -> Result<Route> {
    let only_https = base.https == HttpsPolicy::Only;
    let listen = match listen(env, base.kind, only_https) {
        Ok(listen) => listen,
        Err(e) => return Err(anyhow!("listen: {e}")),
    };
    let cache = match cache(env) {
        Ok(cache) => cache,
        Err(e) => return Err(anyhow!("cache: {e}")),
    };
    let tls = if only_https {
        match tls(app, paths) {
            Ok(found) => found,
            Err(e) => return Err(anyhow!("tls: {e}")),
        }
    } else {
        None
    };
    let snippet = match riku_nginx::app_snippet(env, app_path, paths) {
        Ok(snippet) => snippet,
        Err(e) => return Err(anyhow!("snippet: {e}")),
    };
    Ok(Route {
        listen,
        tls,
        static_paths: riku_nginx::static_locations(env),
        cache,
        cloudflare_acl: flag(env, "RIKU_NGINX_CLOUDFLARE_ACL"),
        allow_git_folders: flag(env, "RIKU_NGINX_ALLOW_GIT_FOLDERS"),
        catch_all: text_key(env, "RIKU_NGINX_CATCH_ALL"),
        csp: text_key(env, "RIKU_NGINX_CSP"),
        snippet,
        ..base
    })
}

/// The route for `app` with one backend per entry of `backends`, checked by
/// the `riku_route` gate for the active router.
///
/// `None` means a Caddy app that sets no domain: it is not routed, and the
/// caller removes its site as `configure_caddy` does today. nginx always gets
/// a route, with `<app>.example.com` when `RIKU_DOMAINS` is unset.
///
/// `RIKU_NGINX_CACHE_PATH` and `RIKU_NGINX_SOCKET` are refused: the helper
/// picks both paths. `RIKU_NGINX_DOCUMENT_ROOT` and the sources in
/// `RIKU_NGINX_STATIC_PATHS` only steer what a deploy publishes, not the
/// route. A certificate and key found in `nginx_root` ride along when the app
/// is `https_only`.
pub fn build_route(
    app: &AppName,
    app_path: &Path,
    env: &HashMap<String, String>,
    backends: &[Backend],
    paths: &RikuPaths,
    router: &Router,
) -> Result<Option<Route>> {
    if let Some((key, why)) = refused_key(env) {
        return Err(route_error(
            app,
            anyhow!("{key} is no longer supported, {why}"),
        ));
    }
    let caddy = matches!(router, Router::Caddy);
    let domains = if caddy {
        caddy_domains(env)
    } else {
        nginx_domains(app, env)
    };
    let domains = match domains {
        Ok(domains) => domains,
        Err(e) => return Err(route_error(app, e)),
    };
    let kind = site_kind(env, router);
    if caddy && domains.is_empty() && kind == SiteKind::Plain {
        return Ok(None);
    }
    let backends = match route_backends(app, backends, paths) {
        Ok(backends) => backends,
        Err(e) => return Err(route_error(app, e)),
    };
    let https = if https_only(env) {
        HttpsPolicy::Only
    } else {
        HttpsPolicy::AlsoPlainHttp
    };
    let base = caddy_route(domains, https, backends, kind);
    if caddy {
        return match check_caddy_route(app, &base) {
            Ok(checked) => Ok(Some(checked.into_route())),
            Err(e) => Err(e),
        };
    }
    match nginx_route(app, app_path, env, base, paths) {
        Ok(route) => Ok(Some(route)),
        Err(e) => Err(route_error(app, e)),
    }
}

#[cfg(test)]
#[path = "route_build_tests.rs"]
mod tests;
