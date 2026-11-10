//! The `riku doctor --fix` step for apps with no site file in the active
//! router's site root. An upgrade removes the site files an older version
//! left behind (`~/.riku/nginx/<app>.conf`, deploy owned `/etc/riku/caddy`
//! files), so every app that has a checkout and no file in the site root is
//! redeployed once and the router helper renders its site.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use super::Check;

const NAME: &str = "router sites";

fn unrouted_apps(riku_root: &Path, site_root: &Path, extension: &str) -> Result<Vec<String>> {
    let apps_root = riku_root.join("apps");
    let entries = match fs::read_dir(&apps_root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", apps_root.display())),
    };
    let mut apps = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => return Err(anyhow!("cannot read an entry of the apps directory: {e}")),
        };
        let app = match entry.file_name().to_str() {
            Some(app) => app.to_string(),
            None => continue,
        };
        if riku_container::types::AppName::try_from(app.as_str()).is_err() || !entry.path().is_dir()
        {
            continue;
        }
        if !site_root.join(format!("{app}.{extension}")).exists() {
            apps.push(app);
        }
    }
    apps.sort();
    Ok(apps)
}

/// Redeploy every app that has a checkout under `<riku_root>/apps` and no
/// `<site_root>/<app>.<extension>`, through `redeploy`, and report one check
/// per app. Nothing is reported when every app has its site file.
/// `wants_site` leaves out apps the router keeps no site for (path mode, a
/// Caddy app with no domain), which would otherwise redeploy on every run.
pub fn redeploy_unrouted_sites(
    riku_root: &Path,
    site_root: &Path,
    extension: &str,
    wants_site: impl Fn(&str) -> bool,
    mut redeploy: impl FnMut(&str) -> Result<()>,
) -> Vec<Check> {
    let apps = match unrouted_apps(riku_root, site_root, extension) {
        Ok(apps) => apps,
        Err(e) => return vec![Check::fail(NAME, e.to_string())],
    };
    apps.iter()
        .filter(|app| wants_site(app))
        .map(|app| match redeploy(app) {
            Ok(()) => Check::ok(
                NAME,
                format!("redeployed {app} into {}", site_root.display()),
            ),
            Err(e) => Check::fail(NAME, format!("redeploying {app} failed: {e}")),
        })
        .collect()
}

#[cfg(test)]
#[path = "redeploy_sites_tests.rs"]
mod tests;
