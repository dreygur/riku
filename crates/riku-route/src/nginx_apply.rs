//! Installing and removing an app's nginx config for the router helper.
//!
//! Security model: this runs as root on a route the deploy user sent. The
//! route passes [`check_nginx_route`] here whatever the sender did, and only
//! the checked route is rendered. Root writes only under the roots in
//! [`NginxRoots`] (`/etc/riku` in production), and every path is the root
//! joined with the checked [`AppName`] and a fixed file name, never request
//! text. TLS material is written from the gate's re-encoded PEM and never
//! read from a path. The helper fetches nothing.
//!
//! A change is all or nothing from nginx's point of view. The previous
//! content of every file it touches is kept, the new files are written,
//! `nginx -t` runs on the result and nginx reloads. If anything after the
//! first write fails, the previous files are put back before the reply, so a
//! refused config is never left for the next reload to pick up.

use std::fs;

use anyhow::Error;
use riku_container::types::AppName;
use riku_util::route_plan::HttpsPolicy;

use crate::check::check_nginx_route;
use crate::nginx_render::{render, CERTIFICATE_FILE, KEY_FILE};
use crate::nginx_roots::NginxRoots;
use crate::nginx_run::{check_config, reload};
use crate::protocol::{Reply, Stage};
use crate::schema::{Route, Tls};
use crate::site_files::{
    ensure_dir, install, Planned, Snapshot, PRIVATE_DIR, PRIVATE_FILE, PUBLIC_DIR, PUBLIC_FILE,
};

pub(crate) fn failed(stage: Stage, error: impl ToString) -> Reply {
    Reply::Failed {
        stage,
        error: error.to_string(),
    }
}

fn plan(
    roots: &NginxRoots,
    app: &AppName,
    config: Option<String>,
    tls: Option<&Tls>,
) -> Vec<Planned> {
    let cert_dir = roots.cert_dir(app);
    vec![
        Planned {
            path: cert_dir.join(KEY_FILE),
            mode: PRIVATE_FILE,
            contents: tls.map(|pair| pair.key_pem.clone().into_bytes()),
        },
        Planned {
            path: cert_dir.join(CERTIFICATE_FILE),
            mode: PUBLIC_FILE,
            contents: tls.map(|pair| pair.cert_pem.clone().into_bytes()),
        },
        Planned {
            path: roots.site_file(app),
            mode: PUBLIC_FILE,
            contents: config.map(String::into_bytes),
        },
    ]
}

fn prepare_dirs(roots: &NginxRoots, app: &AppName, tls: bool) -> Result<(), Error> {
    let mut dirs = vec![(roots.site_root.clone(), PUBLIC_DIR)];
    if tls {
        dirs.push((roots.cert_root.clone(), PRIVATE_DIR));
        dirs.push((roots.cert_dir(app), PRIVATE_DIR));
    }
    dirs.iter()
        .try_for_each(|(dir, mode)| ensure_dir(dir, *mode, roots.owner_uid))
}

/// Run the router's own check and reload on the files now on disk. A failure
/// is the stage it happened at and the router's message.
fn check_and_reload(roots: &NginxRoots) -> Result<(), (Stage, Error)> {
    if let Err(e) = check_config(roots) {
        return Err((Stage::Check, e));
    }
    match reload(roots) {
        Ok(()) => Ok(()),
        Err(e) => Err((Stage::Reload, e)),
    }
}

/// Put the previous files back after `cause` and build the reply, saying so
/// when the restore itself failed.
pub(crate) fn rolled_back(snapshot: &Snapshot, stage: Stage, cause: Error) -> Reply {
    match snapshot.restore() {
        Ok(()) => failed(stage, cause),
        Err(e) => failed(
            stage,
            format!("{cause}; restoring the previous files also failed: {e}"),
        ),
    }
}

/// Write `planned`, check, reload; restore on any failure after the first
/// write.
fn commit(roots: &NginxRoots, planned: &[Planned]) -> Reply {
    let snapshot = match Snapshot::take(planned) {
        Ok(snapshot) => snapshot,
        Err(e) => return failed(Stage::Render, e),
    };
    if let Err(e) = install(planned) {
        return rolled_back(&snapshot, Stage::Render, e);
    }
    match check_and_reload(roots) {
        Ok(()) => Reply::Ok { reloaded: true },
        Err((stage, cause)) => rolled_back(&snapshot, stage, cause),
    }
}

/// Render `route` for `app`, make it live and reload nginx. The reply is
/// [`Reply::Ok`] only once nginx accepted the reload.
pub fn apply_nginx(roots: &NginxRoots, app: &AppName, route: &Route) -> Reply {
    let checked = match check_nginx_route(app, route) {
        Ok(checked) => checked,
        Err(e) => return failed(Stage::Schema, e),
    };
    let route = checked.route();
    let config = match render(roots, app, route) {
        Ok(config) => config,
        Err(e) => return failed(Stage::Render, e),
    };
    let tls = route
        .tls
        .as_ref()
        .filter(|_| route.https == HttpsPolicy::Only);
    if let Err(e) = prepare_dirs(roots, app, tls.is_some()) {
        return failed(Stage::Render, e);
    }
    commit(roots, &plan(roots, app, Some(config), tls))
}

/// Take `app` out of nginx. An app with no config is not an error and does
/// not reload.
pub fn remove_nginx(roots: &NginxRoots, app: &AppName) -> Reply {
    let planned = plan(roots, app, None, None);
    match Snapshot::take(&planned) {
        Ok(snapshot) if snapshot.is_empty() => return Reply::Ok { reloaded: false },
        Ok(_) => {}
        Err(e) => return failed(Stage::Render, e),
    }
    let reply = commit(roots, &planned);
    if matches!(reply, Reply::Ok { .. }) {
        let _ = fs::remove_dir(roots.cert_dir(app));
    }
    reply
}

#[cfg(test)]
#[path = "nginx_apply_tests.rs"]
mod tests;
