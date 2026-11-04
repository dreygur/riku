//! Publishing an app's static files where the nginx worker can read them.
//!
//! The nginx worker has no access to the deploy home, so a deploy copies the
//! `RIKU_NGINX_STATIC` document root and every `RIKU_NGINX_STATIC_PATHS`
//! source out of the checkout into the static root:
//!
//! ```text
//! <static_root>/<app>            symlink to the current copy, <app>~<stamp>
//! <static_root>/<app>/root       the document root
//! <static_root>/<app>/paths/<n>  the n-th static path mapping
//! ```
//!
//! A new copy is staged next to the old one and the `<app>` link is renamed
//! over it, so nginx sees the old files or the new ones, never a half copy.
//! `~` cannot appear in an app name, so no app's entries match another's.
//!
//! Security: the sources come from the app's ENV, which an app repo can
//! set; [`super::static_copy`] keeps every copied path inside the checkout.

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};

use super::static_copy::{copy_source, make_dir};
use crate::config::RikuPaths;

const STATIC_KEY: &str = "RIKU_NGINX_STATIC";
const STATIC_PATHS_KEY: &str = "RIKU_NGINX_STATIC_PATHS";
const DOCUMENT_ROOT_KEY: &str = "RIKU_NGINX_DOCUMENT_ROOT";
const DEFAULT_DOCUMENT_ROOT: &str = "public";

/// One `RIKU_NGINX_STATIC_PATHS` entry, `location:source`.
#[derive(Debug, PartialEq)]
pub(crate) struct StaticPath {
    /// The URL prefix nginx matches.
    pub location: String,
    /// The directory to serve, relative to the app checkout.
    pub source: String,
}

/// The well formed entries of `RIKU_NGINX_STATIC_PATHS`, in order. An entry
/// without exactly one `:` or with an empty side is skipped.
pub(crate) fn static_paths(env: &HashMap<String, String>) -> Vec<StaticPath> {
    let Some(value) = env.get(STATIC_PATHS_KEY) else {
        return Vec::new();
    };
    value
        .split(',')
        .filter_map(|mapping| match mapping.split(':').collect::<Vec<_>>()[..] {
            [location, source] if !location.trim().is_empty() && !source.trim().is_empty() => {
                Some(StaticPath {
                    location: location.trim().to_string(),
                    source: source.trim().to_string(),
                })
            }
            _ => None,
        })
        .collect()
}

fn site_link(app: &str, paths: &RikuPaths) -> PathBuf {
    paths.static_root.join(app)
}

/// Where nginx finds the published document root.
pub(crate) fn document_root(app: &str, paths: &RikuPaths) -> PathBuf {
    site_link(app, paths).join("root")
}

/// Where nginx finds the published copy of the `index`-th static path.
pub(crate) fn static_path_dir(app: &str, paths: &RikuPaths, index: usize) -> PathBuf {
    site_link(app, paths).join("paths").join(index.to_string())
}

/// Copy `app`'s static files from `app_path` into the static root, or
/// remove any earlier copy when the app serves none.
pub(crate) fn publish_static(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let serves_root = env.contains_key(STATIC_KEY);
    let mappings = static_paths(env);
    if !serves_root && mappings.is_empty() {
        return remove_static(app, paths);
    }
    if !paths.static_root.is_dir() {
        return Err(anyhow!(
            "{} does not exist, so nginx cannot serve static files; run `sudo riku init`",
            paths.static_root.display()
        ));
    }
    let checkout = match fs::canonicalize(app_path) {
        Ok(checkout) => checkout,
        Err(e) => return Err(anyhow!("cannot resolve {}: {e}", app_path.display())),
    };

    let document_root = serves_root.then(|| {
        env.get(DOCUMENT_ROOT_KEY)
            .map(String::as_str)
            .unwrap_or(DEFAULT_DOCUMENT_ROOT)
    });
    let stage = paths.static_root.join(format!("{app}~{}", stamp()));
    if let Err(e) = copy_sources(&checkout, document_root, &mappings, &stage) {
        let _ = fs::remove_dir_all(&stage);
        return Err(e);
    }
    swap_in(app, &stage, paths)
}

#[allow(clippy::question_mark)]
fn copy_sources(
    checkout: &Path,
    document_root: Option<&str>,
    mappings: &[StaticPath],
    stage: &Path,
) -> Result<()> {
    if let Err(e) = make_dir(stage) {
        return Err(e);
    }
    if let Some(source) = document_root {
        if let Err(e) = copy_source(checkout, source, &stage.join("root")) {
            return Err(e);
        }
    }
    if mappings.is_empty() {
        return Ok(());
    }
    if let Err(e) = make_dir(&stage.join("paths")) {
        return Err(e);
    }
    for (index, mapping) in mappings.iter().enumerate() {
        let dest = stage.join("paths").join(index.to_string());
        if let Err(e) = copy_source(checkout, &mapping.source, &dest) {
            return Err(e);
        }
    }
    Ok(())
}

fn stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default()
}

/// Point `<static_root>/<app>` at `stage` with one rename, then drop the
/// copies it replaced.
#[allow(clippy::question_mark)]
fn swap_in(app: &str, stage: &Path, paths: &RikuPaths) -> Result<()> {
    let Some(stage_name) = stage.file_name() else {
        return Err(anyhow!("{} has no file name", stage.display()));
    };
    let link = site_link(app, paths);
    let next_link = paths.static_root.join(format!("{app}~link"));
    if let Err(e) = remove_entry(&next_link) {
        return Err(e);
    }
    if let Err(e) = std::os::unix::fs::symlink(stage_name, &next_link) {
        return Err(anyhow!("cannot create {}: {e}", next_link.display()));
    }
    // rename cannot replace a directory with a link.
    if link.symlink_metadata().is_ok_and(|meta| meta.is_dir()) {
        if let Err(e) = remove_entry(&link) {
            return Err(e);
        }
    }
    if let Err(e) = fs::rename(&next_link, &link) {
        return Err(anyhow!("cannot publish {}: {e}", link.display()));
    }
    remove_copies(app, paths, Some(&stage_name.to_string_lossy()))
}

/// Remove `app`'s published static files, if any.
pub fn remove_static(app: &str, paths: &RikuPaths) -> Result<()> {
    match remove_entry(&site_link(app, paths)) {
        Ok(()) => remove_copies(app, paths, None),
        Err(e) => Err(e),
    }
}

/// Remove every `<app>~*` entry in the static root except `keep`.
#[allow(clippy::question_mark)]
fn remove_copies(app: &str, paths: &RikuPaths, keep: Option<&str>) -> Result<()> {
    let entries = match fs::read_dir(&paths.static_root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", paths.static_root.display())),
    };
    let prefix = format!("{app}~");
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&prefix) || keep == Some(name.as_str()) {
            continue;
        }
        if let Err(e) = remove_entry(&entry.path()) {
            return Err(e);
        }
    }
    Ok(())
}

/// Remove a file, symlink or directory tree; a missing one is fine. A
/// symlink is removed itself, never followed.
fn remove_entry(path: &Path) -> Result<()> {
    let removed = match path.symlink_metadata() {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(anyhow!("cannot inspect {}: {e}", path.display())),
    };
    match removed {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot remove {}: {e}", path.display())),
    }
}

#[cfg(test)]
#[path = "static_site_tests.rs"]
mod tests;
