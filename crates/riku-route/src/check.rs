//! The gate every route passes before the helper renders it.

use anyhow::{anyhow, Result};
use riku_container::types::{AppName, CopyIndex};
use riku_util::domain::Domain;
use riku_util::route_plan::HttpsPolicy;

use crate::check_text::location;
use crate::check_tls::check_tls;
use crate::schema::{Listen, Route, RouteBackend, SiteKind, Tls};

const HTTP_PORT: u16 = 80;
const FIRST_UNPRIVILEGED_PORT: u16 = 1024;
const CATCH_ALL_NAME: &str = "_";

/// A route that passed the gate. Its TLS pair is the normalized text, so the
/// only form a caller can write to a file is one built from parsed DER.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckedRoute(Route);

impl CheckedRoute {
    pub fn route(&self) -> &Route {
        &self.0
    }

    pub fn into_route(self) -> Route {
        self.0
    }
}

fn domains(route: &Route) -> Result<()> {
    if route.domains.is_empty() {
        return Err(anyhow!("a route needs at least one domain"));
    }
    route.domains.iter().try_for_each(|raw| {
        if raw == CATCH_ALL_NAME {
            return Ok(());
        }
        match Domain::try_from(raw.as_str()) {
            Ok(domain) if domain.as_str() == raw => Ok(()),
            Ok(_) => Err(anyhow!("domain {raw:?} must be written in lowercase")),
            Err(e) => Err(e),
        }
    })
}

fn backend(backend: &RouteBackend) -> Result<()> {
    match backend {
        RouteBackend::Tcp { host, .. } if !host.is_loopback() => Err(anyhow!(
            "backend {host} is not a loopback address; the router only proxies to this host"
        )),
        RouteBackend::Tcp { port: 0, .. } => Err(anyhow!("backend port 0 is not a port")),
        RouteBackend::UnixCopy(copy) if CopyIndex::new(*copy).is_none() => Err(anyhow!(
            "unix_copy {copy} is not a copy number; copies start at 1"
        )),
        _ => Ok(()),
    }
}

fn backends(route: &Route) -> Result<()> {
    if route.backends.is_empty()
        && !(route.kind == SiteKind::Static && route.https != HttpsPolicy::Only)
    {
        return Err(anyhow!("a route needs at least one backend"));
    }
    route.backends.iter().try_for_each(backend)
}

/// The root nginx master binds this port for the deploy user, so only the
/// HTTP port and unprivileged ports are allowed.
fn listen(route: &Route) -> Result<()> {
    let port = route.listen.port;
    if port != HTTP_PORT && port < FIRST_UNPRIVILEGED_PORT {
        return Err(anyhow!(
            "listen port {port} is privileged; use {HTTP_PORT} or {FIRST_UNPRIVILEGED_PORT} and above"
        ));
    }
    Ok(())
}

fn locations(route: &Route) -> Result<()> {
    route
        .static_paths
        .iter()
        .try_for_each(|prefix| location("static_paths entry", prefix))
}

fn common(route: &Route) -> Result<()> {
    let checks: [fn(&Route) -> Result<()>; 4] = [domains, backends, listen, locations];
    checks.iter().try_for_each(|check| check(route))
}

fn normalized_tls(route: &Route) -> Result<Option<Tls>> {
    match &route.tls {
        Some(pair) => check_tls(pair).map(Some),
        None => Ok(None),
    }
}

fn checked(route: &Route, tls: Option<Tls>) -> CheckedRoute {
    CheckedRoute(Route {
        tls,
        ..route.clone()
    })
}

/// Refuse a route that is not safe to render for nginx, naming the app and
/// the field. The first problem found is the one reported. `https: only`
/// needs a TLS pair.
pub fn check_nginx_route(app: &AppName, route: &Route) -> Result<CheckedRoute> {
    let tls = common(route)
        .and_then(|()| normalized_tls(route))
        .and_then(|tls| match (route.https, &tls) {
            (HttpsPolicy::Only, None) => Err(anyhow!("https is only but the route has no tls")),
            _ => Ok(tls),
        });
    match tls {
        Ok(tls) => Ok(checked(route, tls)),
        Err(e) => Err(anyhow!("route for '{app}': {e}")),
    }
}

fn caddy_cannot_take(route: &Route) -> Option<&'static str> {
    let unix_backend = route
        .backends
        .iter()
        .any(|b| matches!(b, RouteBackend::UnixCopy(_) | RouteBackend::UnixDefault));
    let refused = [
        (route.tls.is_some(), "a tls pair"),
        (route.allow_git_folders, "allow_git_folders"),
        (route.listen != Listen::default(), "a custom listen"),
        (
            route.kind == SiteKind::Wsgi || unix_backend,
            "unix socket backends",
        ),
        (route.kind == SiteKind::Portmap, "port mappings"),
        (
            route.kind == SiteKind::Static || !route.static_paths.is_empty(),
            "static sites",
        ),
        (
            route.domains.iter().any(|d| d == CATCH_ALL_NAME),
            "the _ name",
        ),
    ];
    refused.iter().find(|(set, _)| *set).map(|(_, what)| *what)
}

/// The common checks plus what Caddy cannot do: it takes none of the nginx
/// only fields, no unix socket backends and no static sites, only TCP
/// proxying with its own certificates.
pub fn check_caddy_route(app: &AppName, route: &Route) -> Result<CheckedRoute> {
    let outcome = match caddy_cannot_take(route) {
        Some(what) => Err(anyhow!("Caddy does not take {what}")),
        None => common(route),
    };
    match outcome {
        Ok(()) => Ok(checked(route, None)),
        Err(e) => Err(anyhow!("route for '{app}': {e}")),
    }
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
