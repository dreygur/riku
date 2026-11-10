//! Installing and removing an app's Caddy site for the router helper.
//!
//! Security model: this runs as root on a route the deploy user sent. The
//! route passes [`check_caddy_route`] here whatever the sender did, so a
//! unix socket backend or a mismatched TLS pair is refused before any file
//! is written. Root writes `<site root>/<app>.caddy`, named from the checked
//! [`AppName`], with the text `riku-caddy` renders from typed domains and
//! loopback backends, and, for an app with its own certificate, the pair
//! under `<site root>/certs/<app>/` (see `caddy_certs`), from the gate's
//! re-encoded PEM. The reload goes to Caddy's admin API on a
//! unix socket only `caddy` and root reach, so no other local process
//! controls routes. The deploy user never runs `caddy`.
//!
//! A failed reload puts the previous site file back. Caddy keeps serving its
//! old config when a reload is refused, so nothing else needs undoing.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, Error, Result};
use riku_caddy::{render_site_with_tls, ADMIN_ADDRESS, SYSTEM_CADDYFILE};
use riku_config::{ReloadPrograms, CADDY_SITE_ROOT};
use riku_container::types::AppName;
use riku_util::backend::Backend;
use riku_util::domain::Domain;
use riku_util::listening::listening_ports;
use riku_util::route_plan::RoutePlan;
use riku_util::write_dir::{check_write_dir, ROOT_UID};

use crate::answer::failed;
use crate::caddy_certs::{planned_pair, prepare_dirs, remove_dir};
use crate::check::check_caddy_route;
use crate::nginx_apply::rolled_back;
use crate::nginx_run::run;
use crate::port_wait::wait_for_port;
use crate::protocol::{Reply, Stage};
use crate::schema::{Route, RouteBackend, Tls};
use crate::site_files::{install, Planned, Snapshot};

/// Readable by the `caddy` group through the site directory's group, which
/// root init sets on the directory with the setgid bit.
pub(crate) const SITE_FILE: u32 = 0o640;

/// Where the helper keeps Caddy's site files and which program it runs.
#[derive(Debug, Clone)]
pub struct CaddyRoots {
    pub site_root: PathBuf,
    pub caddy: PathBuf,
    /// The Caddyfile that imports the site files.
    pub caddyfile: PathBuf,
    /// Caddy's admin address, as `caddy reload --address` takes it.
    pub admin_address: String,
    /// The uid the site directory must belong to: root in production, the
    /// test's own uid in tests.
    pub owner_uid: u32,
}

impl CaddyRoots {
    /// The roots of a real host, with `caddy` looked up on `PATH`.
    pub fn production() -> Self {
        CaddyRoots {
            site_root: PathBuf::from(CADDY_SITE_ROOT),
            caddy: ReloadPrograms::default().caddy,
            caddyfile: PathBuf::from(SYSTEM_CADDYFILE),
            admin_address: ADMIN_ADDRESS.to_string(),
            owner_uid: ROOT_UID,
        }
    }

    fn site_file(&self, app: &AppName) -> PathBuf {
        self.site_root.join(format!("{app}.caddy"))
    }
}

pub(crate) fn backend(backend: &RouteBackend) -> Result<Backend> {
    match backend {
        RouteBackend::Tcp { host, port } => Ok(Backend::Tcp {
            host: *host,
            port: *port,
        }),
        RouteBackend::UnixCopy(_) | RouteBackend::UnixDefault => {
            Err(anyhow!("Caddy proxies TCP only, not unix sockets"))
        }
    }
}

fn plan_of(route: &Route) -> Result<RoutePlan> {
    let domains: Result<Vec<Domain>> = route
        .domains
        .iter()
        .map(|raw| Domain::try_from(raw.as_str()))
        .collect();
    let backends: Result<Vec<Backend>> = route.backends.iter().map(backend).collect();
    match (domains, backends) {
        (Ok(domains), Ok(backends)) => Ok(RoutePlan {
            domains,
            https: route.https,
            backends,
            open_port: route.open_port,
        }),
        (Err(e), _) | (_, Err(e)) => Err(e),
    }
}

fn reload(roots: &CaddyRoots) -> Result<()> {
    run(
        Command::new(&roots.caddy)
            .arg("reload")
            .arg("--config")
            .arg(&roots.caddyfile)
            .args(["--adapter", "caddyfile", "--address"])
            .arg(&roots.admin_address),
        "caddy reload",
    )
}

/// Write `planned`, reload Caddy, wait for `open_port` to be listened on,
/// and restore the previous files when either fails.
pub(crate) fn commit(
    roots: &CaddyRoots,
    planned: &[Planned],
    open_port: Option<u16>,
    probe: impl Fn() -> BTreeSet<u16>,
) -> Reply {
    let snapshot = match Snapshot::take(planned) {
        Ok(snapshot) => snapshot,
        Err(e) => return failed(Stage::Render, e),
    };
    if let Err(e) = install(planned) {
        return rolled_back(&snapshot, Stage::Render, e);
    }
    if let Err(e) = reload(roots) {
        return rolled_back(&snapshot, Stage::Reload, e);
    }
    match open_port.map(|port| wait_for_port(port, probe)) {
        Some(Err(cause)) => rolled_back(&snapshot, Stage::Reload, Error::msg(cause)),
        _ => Reply::Ok { reloaded: true },
    }
}

fn plan(
    roots: &CaddyRoots,
    app: &AppName,
    site: Option<String>,
    tls: Option<&Tls>,
) -> Vec<Planned> {
    let site = Planned {
        path: roots.site_file(app),
        mode: SITE_FILE,
        contents: site.map(String::into_bytes),
    };
    let [key, certificate] = planned_pair(roots, app, tls);
    vec![key, certificate, site]
}

/// Render `route` for `app`, make it live and reload Caddy. The reply is
/// [`Reply::Ok`] only once Caddy accepted the reload.
pub fn apply_caddy(roots: &CaddyRoots, app: &AppName, route: &Route) -> Reply {
    let checked = match check_caddy_route(app, route) {
        Ok(checked) => checked,
        Err(e) => return failed(Stage::Schema, e),
    };
    let route = checked.route();
    let files = route.tls.as_ref().map(|_| roots.tls_files(app));
    let site = match plan_of(route)
        .and_then(|plan| render_site_with_tls(app.as_str(), &plan, files.as_ref()))
    {
        Ok(site) => site,
        Err(e) => return failed(Stage::Render, e),
    };
    if let Err(e) = check_write_dir(&roots.site_root, roots.owner_uid) {
        return failed(Stage::Render, e);
    }
    if route.tls.is_some() {
        if let Err(e) = prepare_dirs(roots, app) {
            return failed(Stage::Render, e);
        }
    }
    let planned = plan(roots, app, Some(site), route.tls.as_ref());
    let reply = commit(roots, &planned, route.open_port, listening_ports);
    if route.tls.is_none() && matches!(reply, Reply::Ok { .. }) {
        remove_dir(roots, app);
    }
    reply
}

/// Take `app` out of Caddy. An app with no site is not an error and does not
/// reload.
pub fn remove_caddy(roots: &CaddyRoots, app: &AppName) -> Reply {
    let planned = plan(roots, app, None, None);
    match Snapshot::take(&planned) {
        Ok(snapshot) if snapshot.is_empty() => return Reply::Ok { reloaded: false },
        Ok(_) => {}
        Err(e) => return failed(Stage::Render, e),
    }
    if let Err(e) = check_write_dir(&roots.site_root, roots.owner_uid) {
        return failed(Stage::Render, e);
    }
    let reply = commit(roots, &planned, None, BTreeSet::new);
    if matches!(reply, Reply::Ok { .. }) {
        remove_dir(roots, app);
    }
    reply
}

#[cfg(test)]
#[path = "caddy_apply_tests.rs"]
mod tests;
