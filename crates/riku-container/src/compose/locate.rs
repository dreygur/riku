//! Locating a compose app's compose file.
//!
//! `COMPOSE_FILE` in the app's ENV picks a compose file under another name.
//! It is user input that becomes a `-f` argument, so it must be one relative
//! path that resolves to a file inside the app directory.

use anyhow::{bail, Result};
use std::path::{Component, Path, PathBuf};

/// Default compose file names, in docker compose's own precedence order.
pub const COMPOSE_FILE_NAMES: [&str; 4] = [
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

/// A compose file path relative to the app directory, checked to stay inside it.
///
/// Every compose command runs with the app directory as its working directory,
/// so this relative form is what gets passed to `-f`.
#[derive(Debug, PartialEq)]
pub struct ComposeFile(PathBuf);

impl ComposeFile {
    /// The path as a `-f` argument.
    pub fn arg(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    /// The path relative to the app directory.
    pub fn relative(&self) -> &Path {
        &self.0
    }
}

/// First of the default compose file names present, following docker
/// compose's own precedence.
pub fn default_compose_file(app_path: &Path) -> Option<ComposeFile> {
    COMPOSE_FILE_NAMES
        .iter()
        .find(|name| app_path.join(name).exists())
        .map(|name| ComposeFile(PathBuf::from(name)))
}

/// Check that a `COMPOSE_FILE` value names exactly one file inside the app.
///
/// The value comes from the app's ENV, which is user input, so it is rejected
/// when it is absolute, climbs out with `..`, lists several files the way
/// docker compose allows (`a.yml:b.yml`), or resolves through a symlink to a
/// file outside the app directory.
fn validate_compose_file(app_path: &Path, requested: &str) -> Result<ComposeFile> {
    if requested.contains(':') {
        bail!("COMPOSE_FILE='{requested}' lists several files; riku supports exactly one");
    }

    let relative = Path::new(requested);
    let is_plain_relative = relative
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if !is_plain_relative {
        bail!("COMPOSE_FILE='{requested}' must be a relative path inside the app directory");
    }

    let app_root = match app_path.canonicalize() {
        Ok(path) => path,
        Err(e) => bail!("cannot resolve app directory {}: {e}", app_path.display()),
    };
    let resolved = match app_path.join(relative).canonicalize() {
        Ok(path) => path,
        Err(e) => bail!("COMPOSE_FILE='{requested}' not found in the app: {e}"),
    };
    if !resolved.starts_with(&app_root) || !resolved.is_file() {
        bail!("COMPOSE_FILE='{requested}' must point to a file inside the app directory");
    }

    Ok(ComposeFile(relative.to_path_buf()))
}

/// Find the app's compose file: `requested` (the app's `COMPOSE_FILE` value,
/// read by the caller) when given, otherwise the first default name present.
pub fn find_compose_file(app_path: &Path, requested: Option<&str>) -> Result<Option<ComposeFile>> {
    match requested {
        Some(name) => validate_compose_file(app_path, name).map(Some),
        None => Ok(default_compose_file(app_path)),
    }
}

#[cfg(test)]
#[path = "locate_tests.rs"]
mod tests;
