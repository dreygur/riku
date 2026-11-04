//! Symlinks in `nginx_sites_enabled` that concern riku.
//!
//! Older riku versions linked each `<app>.conf` there. With the include in
//! place such a link loads the site a second time, and nginx refuses the
//! duplicate `upstream`. Only a link whose resolved target lies inside
//! `nginx_root` is riku's to remove. A link that merely shares an app's
//! name may be an admin's own and is reported, never removed.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::config::RikuPaths;

/// Whether `file_name` is `<app>.conf` for an app that exists.
pub(crate) fn is_app_config(file_name: &str, paths: &RikuPaths) -> bool {
    match file_name.strip_suffix(".conf") {
        Some(app) if !app.is_empty() && !app.contains('/') => paths.app_root.join(app).is_dir(),
        _ => false,
    }
}

/// `path` with `.` and `..` folded away, without touching the filesystem.
fn lexically_normal(path: &Path) -> PathBuf {
    path.components().fold(PathBuf::new(), |mut normal, part| {
        match part {
            Component::ParentDir => {
                normal.pop();
            }
            Component::CurDir => {}
            other => normal.push(other),
        }
        normal
    })
}

/// The link's target, relative targets resolved against its directory, or
/// `None` when `link` is not a symlink.
fn resolved_target(link: &Path) -> Option<PathBuf> {
    let Ok(target) = fs::read_link(link) else {
        return None;
    };
    match link.parent() {
        Some(dir) if target.is_relative() => Some(lexically_normal(&dir.join(&target))),
        _ => Some(lexically_normal(&target)),
    }
}

fn site_links(paths: &RikuPaths) -> Vec<(PathBuf, PathBuf)> {
    let Ok(entries) = fs::read_dir(&paths.nginx_sites_enabled) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter_map(|link| resolved_target(&link).map(|target| (link, target)))
        .collect()
}

/// Links an older riku made: their target lies inside `nginx_root`.
pub fn legacy_site_links(paths: &RikuPaths) -> Vec<PathBuf> {
    site_links(paths)
        .into_iter()
        .filter(|(_, target)| target.starts_with(&paths.nginx_root))
        .map(|(link, _)| link)
        .collect()
}

/// Links named `<app>.conf` for an existing app that point outside
/// `nginx_root`. They may serve the same domain as the app, so doctor
/// reports them, but they are not riku's and stay.
pub fn app_named_links(paths: &RikuPaths) -> Vec<PathBuf> {
    site_links(paths)
        .into_iter()
        .filter(|(link, target)| {
            let named = link
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| is_app_config(name, paths));
            named && !target.starts_with(&paths.nginx_root)
        })
        .map(|(link, _)| link)
        .collect()
}

#[cfg(test)]
#[path = "host_links_tests.rs"]
mod tests;
