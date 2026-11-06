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
/// compose's own precedence. A symlink counts as present, even a dangling one,
/// so that [`find_compose_file`] reports it instead of skipping to the next
/// name.
pub fn default_compose_file(app_path: &Path) -> Option<ComposeFile> {
    COMPOSE_FILE_NAMES
        .iter()
        .find(|name| std::fs::symlink_metadata(app_path.join(name)).is_ok())
        .map(|name| ComposeFile(PathBuf::from(name)))
}

/// Check that a `COMPOSE_FILE` value names exactly one file inside the app.
///
/// The value comes from the app's ENV, which is user input, so it is rejected
/// when it is absolute, climbs out with `..`, lists several files the way
/// docker compose allows (`a.yml:b.yml`), or holds a line break or NUL (it is
/// printed as a `KEY=value` line). The compose file, or a folder on its path,
/// may be a symlink as long as its real target stays inside the app; a link
/// that leaves the app is refused naming the path.
///
/// Only the file itself may be reached through a link. Bind sources, the build
/// context, the dockerfile and `env_file` still refuse every symlink.
fn validate_compose_file(app_path: &Path, requested: &str) -> Result<ComposeFile> {
    if requested.contains(':') {
        bail!("COMPOSE_FILE='{requested}' lists several files; riku supports exactly one");
    }
    if requested.is_empty() || requested.contains(['\n', '\r', '\0']) {
        bail!("COMPOSE_FILE={requested:?} is empty or contains a line break or NUL");
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
    // A plain relative path can only leave the app through a symlink.
    if !resolved.starts_with(&app_root) {
        bail!(
            "compose file '{requested}' is reached through a symlink that leaves the app directory (it resolves to {})",
            resolved.display()
        );
    }
    if !resolved.is_file() {
        bail!("COMPOSE_FILE='{requested}' must point to a file inside the app directory");
    }

    Ok(ComposeFile(relative.to_path_buf()))
}

/// Find the app's compose file: `requested` (the app's `COMPOSE_FILE` value,
/// read by the caller) when given, otherwise the first default name present.
pub fn find_compose_file(app_path: &Path, requested: Option<&str>) -> Result<Option<ComposeFile>> {
    match requested {
        Some(name) => validate_compose_file(app_path, name).map(Some),
        None => match default_compose_file(app_path) {
            Some(default) => validate_compose_file(app_path, &default.arg()).map(Some),
            None => Ok(None),
        },
    }
}

#[cfg(test)]
#[path = "locate_tests.rs"]
mod tests;
