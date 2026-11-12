//! Building the route the router helper accepts from an app's ENV, its
//! running copies and the certificates it already has.
//!
//! Security: the route is built from the app's own ENV, which an app repo
//! can set, so this module only parses; the `riku_route` gate for the active
//! router runs on the result for an early error and again in the helper. No
//! ENV value becomes a path: a key that names one is refused by name, and a
//! unix backend is only accepted as one of the app's own sockets, then sent
//! as a copy number or as the default socket.
//!
//! A Caddy route carries only what Caddy uses (domains, https policy, TCP
//! backends, and the app's own certificate if it has one); the nginx only
//! fields stay unset.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;

use anyhow::{anyhow, Result};
use riku_container::types::AppName;
use riku_route::{check_caddy_route, Listen, Route, RouteBackend, SiteKind};

use super::route_env::{open_port, tls};
use crate::config::own_cert::has_own_cert;
use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::backend::Backend;
use crate::util::domain::{parse_domains, Domain};
use crate::util::file_names::copy_socket;
use crate::util::flag::{https_only, is_on};
use crate::util::route_plan::HttpsPolicy;

pub(super) const CATCH_ALL_NAME: &str = "_";
const REFUSED_KEYS: [(&str, &str); 1] = [("RIKU_NGINX_SOCKET", "the helper picks socket paths")];

fn route_error(app: &AppName, cause: anyhow::Error) -> anyhow::Error {
    anyhow!("cannot build the route of '{app}': {cause}")
}

/// The reason an ENV key set by the app fails the deploy, if one does.
fn refused_key(env: &HashMap<String, String>) -> Option<String> {
    REFUSED_KEYS
        .iter()
        .find(|(key, _)| env.contains_key(*key))
        .map(|(key, why)| format!("{key} cannot be set by an app, {why}"))
}

/// What nginx's generator would serve: the listed names, `_` as the catch
/// all name, or none for a port mode app. An app with neither is an error;
/// after routing that is a uWSGI or static site, which has no port mode.
pub(super) fn nginx_domains(app: &AppName, env: &HashMap<String, String>) -> Result<Vec<String>> {
    let Some(raw) = env.get("RIKU_DOMAINS") else {
        if env.contains_key("RIKU_ROUTING_PORT") {
            return Ok(Vec::new());
        }
        return Err(anyhow!(
            "'{app}' has no domain to route: set routing.domains, or \
             routing.fallback_domain in settings.toml"
        ));
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

pub(super) fn route_backends(
    app: &AppName,
    backends: &[Backend],
    paths: &RikuPaths,
) -> Result<Vec<RouteBackend>> {
    backends
        .iter()
        .map(|backend| route_backend(app, backend, paths))
        .collect()
}

fn flag(env: &HashMap<String, String>, key: &str) -> bool {
    env.get(key).is_some_and(|value| is_on(value))
}

/// Fails for a setting the Caddy route would otherwise drop without a
/// word: `routing.static_paths`, since Caddy serves no static files.
pub(super) fn refuse_nginx_only_settings(env: &HashMap<String, String>) -> Result<()> {
    let static_paths = env
        .get("RIKU_NGINX_STATIC_PATHS")
        .is_some_and(|value| !value.trim().is_empty());
    if static_paths {
        return Err(anyhow!(
            "routing.static_paths is nginx only: Caddy serves no static files. \
             Remove it, or switch the server router to nginx"
        ));
    }
    Ok(())
}

/// A route with only what Caddy uses.
fn caddy_route(
    domains: Vec<String>,
    https: HttpsPolicy,
    backends: Vec<RouteBackend>,
    kind: SiteKind,
    open_port: Option<u16>,
) -> Route {
    Route {
        domains,
        https,
        backends,
        listen: Listen::default(),
        tls: None,
        static_paths: Vec::new(),
        kind,
        allow_git_folders: false,
        open_port,
    }
}

/// Caddy issues its own certificate, so the route carries a pair only when
/// the app supplied one.
fn caddy_tls(app: &AppName, base: Route, paths: &RikuPaths) -> Result<Route> {
    if !has_own_cert(paths, app.as_str()) {
        return Ok(base);
    }
    match tls(app, paths) {
        Ok(Some(pair)) => Ok(Route {
            tls: Some(pair),
            ..base
        }),
        Ok(None) => Err(anyhow!(
            "tls: '{app}' is marked as having its own certificate but its pair is missing from {}",
            paths.nginx_root.display()
        )),
        Err(e) => Err(anyhow!("tls: {e}")),
    }
}

fn nginx_route(
    app: &AppName,
    env: &HashMap<String, String>,
    base: Route,
    paths: &RikuPaths,
) -> Result<Route> {
    let own = has_own_cert(paths, app.as_str());
    let wanted = base.https == HttpsPolicy::Only || own;
    let tls = if wanted {
        match tls(app, paths) {
            Ok(None) if own => {
                return Err(anyhow!(
                    "tls: '{app}' is marked as having its own certificate but its pair is missing from {}",
                    paths.nginx_root.display()
                ))
            }
            Ok(found) => found,
            Err(e) => return Err(anyhow!("tls: {e}")),
        }
    } else {
        None
    };
    Ok(Route {
        tls,
        static_paths: riku_nginx::static_locations(env),
        allow_git_folders: flag(env, "RIKU_NGINX_ALLOW_GIT_FOLDERS"),
        ..base
    })
}

/// The route for `app` with one backend per entry of `backends`, checked by
/// the `riku_route` gate for the active router.
///
/// `None` means a Caddy app that sets no domain: it is not routed, and the
/// caller removes its site as `configure_caddy` does today. nginx always gets
/// a route; see [`nginx_domains`] for an app with no domain and no port.
///
/// An ENV key that names a path the helper picks (`RIKU_NGINX_SOCKET`)
/// fails the build, naming the key.
/// `RIKU_NGINX_DOCUMENT_ROOT` and the sources in `RIKU_NGINX_STATIC_PATHS`
/// only steer what a deploy publishes, not the route. A certificate and key
/// found in `nginx_root` ride along when the app is `https_only`, and, for
/// nginx and Caddy alike, whenever the app has an own certificate marker.
pub fn build_route(
    app: &AppName,
    env: &HashMap<String, String>,
    backends: &[Backend],
    paths: &RikuPaths,
    router: &Router,
) -> Result<Option<Route>> {
    if let Some(reason) = refused_key(env) {
        return Err(route_error(app, anyhow!(reason)));
    }
    let caddy = matches!(router, Router::Caddy);
    if caddy {
        if let Err(e) = refuse_nginx_only_settings(env) {
            return Err(route_error(app, e));
        }
    }
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
    let port = match open_port(env) {
        Ok(port) => port,
        Err(e) => return Err(route_error(app, e)),
    };
    if caddy && domains.is_empty() && kind == SiteKind::Plain && port.is_none() {
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
    let base = caddy_route(domains, https, backends, kind, port);
    if caddy {
        let base = match caddy_tls(app, base, paths) {
            Ok(route) => route,
            Err(e) => return Err(route_error(app, e)),
        };
        return match check_caddy_route(app, &base) {
            Ok(checked) => Ok(Some(checked.into_route())),
            Err(e) => Err(e),
        };
    }
    match nginx_route(app, env, base, paths) {
        Ok(route) => Ok(Some(route)),
        Err(e) => Err(route_error(app, e)),
    }
}

#[cfg(test)]
#[path = "route_build_tests.rs"]
mod tests;
