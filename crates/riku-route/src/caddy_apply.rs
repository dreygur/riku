//! Installing and removing an app's Caddy site for the router helper.
//!
//! Security model: this runs as root on a route the deploy user sent. The
//! route passes [`check_caddy_route`] here whatever the sender did, so a
//! snippet, a TLS pair or a unix socket backend is refused before any file
//! is written. Root writes one file, `<site root>/<app>.caddy`, named from
//! the checked [`AppName`], with the text `riku-caddy` renders from typed
//! domains and loopback backends. The reload goes to Caddy's admin API on a
//! unix socket only `caddy` and root reach, so no other local process
//! controls routes. The deploy user never runs `caddy`.
//!
//! A failed reload puts the previous site file back. Caddy keeps serving its
//! old config when a reload is refused, so nothing else needs undoing.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, Result};
use riku_caddy::{render_site, ADMIN_ADDRESS, SYSTEM_CADDYFILE};
use riku_config::{ReloadPrograms, CADDY_SITE_ROOT};
use riku_container::types::AppName;
use riku_util::backend::Backend;
use riku_util::domain::Domain;
use riku_util::route_plan::RoutePlan;
use riku_util::write_dir::{check_write_dir, ROOT_UID};

use crate::check::check_caddy_route;
use crate::nginx_apply::{failed, rolled_back};
use crate::nginx_run::run;
use crate::protocol::{Reply, Stage};
use crate::schema::{Route, RouteBackend};
use crate::site_files::{install, Planned, Snapshot};

/// Readable by the `caddy` group through the site directory's group, which
/// root init sets on the directory with the setgid bit.
const SITE_FILE: u32 = 0o640;

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

fn backend(backend: &RouteBackend) -> Result<Backend> {
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

/// Write `planned`, reload Caddy, and restore the previous files when the
/// reload is refused.
fn commit(roots: &CaddyRoots, planned: &[Planned]) -> Reply {
    let snapshot = match Snapshot::take(planned) {
        Ok(snapshot) => snapshot,
        Err(e) => return failed(Stage::Render, e),
    };
    if let Err(e) = install(planned) {
        return rolled_back(&snapshot, Stage::Render, e);
    }
    match reload(roots) {
        Ok(()) => Reply::Ok { reloaded: true },
        Err(e) => rolled_back(&snapshot, Stage::Reload, e),
    }
}

fn plan(roots: &CaddyRoots, app: &AppName, site: Option<String>) -> Vec<Planned> {
    vec![Planned {
        path: roots.site_file(app),
        mode: SITE_FILE,
        contents: site.map(String::into_bytes),
    }]
}

/// Render `route` for `app`, make it live and reload Caddy. The reply is
/// [`Reply::Ok`] only once Caddy accepted the reload.
pub fn apply_caddy(roots: &CaddyRoots, app: &AppName, route: &Route) -> Reply {
    let checked = match check_caddy_route(app, route) {
        Ok(checked) => checked,
        Err(e) => return failed(Stage::Schema, e),
    };
    let site = match plan_of(checked.route()).and_then(|plan| render_site(app.as_str(), &plan)) {
        Ok(site) => site,
        Err(e) => return failed(Stage::Render, e),
    };
    if let Err(e) = check_write_dir(&roots.site_root, roots.owner_uid) {
        return failed(Stage::Render, e);
    }
    commit(roots, &plan(roots, app, Some(site)))
}

/// Take `app` out of Caddy. An app with no site is not an error and does not
/// reload.
pub fn remove_caddy(roots: &CaddyRoots, app: &AppName) -> Reply {
    let planned = plan(roots, app, None);
    match Snapshot::take(&planned) {
        Ok(snapshot) if snapshot.is_empty() => return Reply::Ok { reloaded: false },
        Ok(_) => {}
        Err(e) => return failed(Stage::Render, e),
    }
    if let Err(e) = check_write_dir(&roots.site_root, roots.owner_uid) {
        return failed(Stage::Render, e);
    }
    commit(roots, &planned)
}

#[cfg(test)]
#[path = "caddy_apply_tests.rs"]
mod tests;
