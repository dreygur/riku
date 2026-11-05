//! The `riku doctor --fix` step for apps still routed from the old nginx
//! directory. Before the router helper, an app's config was
//! `<riku root>/nginx/<app>.conf`; the helper now writes it to the site root
//! (`/etc/riku/nginx`). An app that has the old file and no new one is
//! redeployed once so the helper renders its site.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Result};

use super::Check;

const NAME: &str = "nginx sites";

fn orphaned_apps(riku_root: &Path, site_root: &Path) -> Result<Vec<String>> {
    let old_root = riku_root.join("nginx");
    let entries = match fs::read_dir(&old_root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", old_root.display())),
    };
    let mut apps = Vec::new();
    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(e) => {
                return Err(anyhow!(
                    "cannot read an entry of the old nginx directory: {e}"
                ))
            }
        };
        let app = match (path.extension(), path.file_stem().and_then(|s| s.to_str())) {
            (Some(ext), Some(stem)) if ext == "conf" => stem,
            _ => continue,
        };
        if !site_root.join(format!("{app}.conf")).exists() {
            apps.push(app.to_string());
        }
    }
    apps.sort();
    Ok(apps)
}

/// Redeploy every app that has `<riku_root>/nginx/<app>.conf` but no
/// `<site_root>/<app>.conf`, through `redeploy`, and report one check per
/// app. Nothing is reported when no app is in that state.
pub fn redeploy_orphaned_sites(
    riku_root: &Path,
    site_root: &Path,
    mut redeploy: impl FnMut(&str) -> Result<()>,
) -> Vec<Check> {
    let apps = match orphaned_apps(riku_root, site_root) {
        Ok(apps) => apps,
        Err(e) => return vec![Check::fail(NAME, e.to_string())],
    };
    apps.iter()
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
