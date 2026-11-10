//! Installing the shared path mode site: `_paths.conf` for nginx and
//! `_paths.caddy` for Caddy, next to the per app site files. `AppName`
//! refuses a leading `_`, so no app's own site file can have these names.
//!
//! Each apply replaces the whole file, reloads, waits for the shared port to
//! be listened on and restores the previous file on any failure, as a single
//! app apply does. An empty list removes the file.

use std::path::PathBuf;

use riku_util::listening::listening_ports;
use riku_util::write_dir::check_write_dir;

use crate::answer::failed;
use crate::caddy_apply::{self, CaddyRoots, SITE_FILE};
use crate::nginx_apply;
use crate::nginx_render;
use crate::nginx_roots::NginxRoots;
use crate::paths_check::check_paths;
use crate::protocol::{Reply, Stage};
use crate::schema::{Listen, PathApp};
use crate::site_files::{Planned, Snapshot, PUBLIC_FILE};

const NGINX_FILE: &str = "_paths.conf";
const CADDY_FILE: &str = "_paths.caddy";

/// The one file to write, or to remove when `contents` is `None`. `None`
/// from this function means there is nothing to do: no apps and no file.
fn planned(
    path: PathBuf,
    mode: u32,
    contents: Option<String>,
) -> Result<Option<Vec<Planned>>, Reply> {
    let planned = vec![Planned {
        path,
        mode,
        contents: contents.map(String::into_bytes),
    }];
    if planned[0].contents.is_some() {
        return Ok(Some(planned));
    }
    match Snapshot::take(&planned) {
        Ok(snapshot) if snapshot.is_empty() => Ok(None),
        Ok(_) => Ok(Some(planned)),
        Err(e) => Err(failed(Stage::Render, e)),
    }
}

/// Install the nginx path site for `apps` on `listen`.
pub fn apply_paths_nginx(roots: &NginxRoots, listen: &Listen, apps: &[PathApp]) -> Reply {
    let checked = match check_paths(listen, apps) {
        Ok(checked) => checked,
        Err(e) => return failed(Stage::Schema, e),
    };
    let entries: Vec<riku_nginx::PathApp> = checked
        .iter()
        .map(|(app, backends)| riku_nginx::PathApp {
            app: app.to_string(),
            backends: backends
                .iter()
                .map(|each| nginx_render::backend(roots, app, each))
                .collect(),
        })
        .collect();
    let contents = if entries.is_empty() {
        None
    } else {
        let site = riku_nginx::PathsSite {
            port: listen.port,
            ipv6: roots.ipv6,
            apps: &entries,
        };
        match riku_nginx::render_paths_site(&site) {
            Ok(text) => Some(text),
            Err(e) => return failed(Stage::Render, e),
        }
    };
    let open_port = contents.as_ref().map(|_| listen.port);
    match planned(roots.site_root.join(NGINX_FILE), PUBLIC_FILE, contents) {
        Ok(Some(planned)) => nginx_apply::commit(roots, &planned, open_port, listening_ports),
        Ok(None) => Reply::Ok { reloaded: false },
        Err(reply) => reply,
    }
}

/// Install the Caddy path site for `apps` on `listen`.
pub fn apply_paths_caddy(roots: &CaddyRoots, listen: &Listen, apps: &[PathApp]) -> Reply {
    let checked = match check_paths(listen, apps) {
        Ok(checked) => checked,
        Err(e) => return failed(Stage::Schema, e),
    };
    let mut entries = Vec::with_capacity(checked.len());
    for (app, backends) in &checked {
        let converted: anyhow::Result<Vec<_>> = backends.iter().map(caddy_apply::backend).collect();
        match converted {
            Ok(converted) => entries.push((app.to_string(), converted)),
            Err(e) => return failed(Stage::Render, e),
        }
    }
    let contents = if entries.is_empty() {
        None
    } else {
        match riku_caddy::render_paths(listen.port, &entries) {
            Ok(text) => Some(text),
            Err(e) => return failed(Stage::Render, e),
        }
    };
    if let Err(e) = check_write_dir(&roots.site_root, roots.owner_uid) {
        return failed(Stage::Render, e);
    }
    let open_port = contents.as_ref().map(|_| listen.port);
    match planned(roots.site_root.join(CADDY_FILE), SITE_FILE, contents) {
        Ok(Some(planned)) => caddy_apply::commit(roots, &planned, open_port, listening_ports),
        Ok(None) => Reply::Ok { reloaded: false },
        Err(reply) => reply,
    }
}

#[cfg(test)]
#[path = "paths_apply_tests.rs"]
mod tests;
