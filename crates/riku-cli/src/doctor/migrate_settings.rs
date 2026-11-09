//! `riku doctor --fix` migration from the old config homes: the `ROUTER` and
//! `CONTAINER_ENGINE` files into settings.toml, and each app's riku keys
//! from ENV into its server override (`migrate_env.rs`).
//!
//! Idempotent: a second run finds nothing to move. Skipped as root, because
//! files written as root in the deploy user's tree would be unreadable to it.

#![allow(clippy::question_mark)]

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::router::Router;
use crate::config::{settings, RikuPaths};

use super::{migrate_env, Check};

const NAME: &str = "settings migration";

/// A legacy single value file and the settings key that replaces it.
struct LegacyFile<'a> {
    path: &'a Path,
    section: &'static str,
    key: &'static str,
}

fn current_value(paths: &RikuPaths, section: &str) -> Result<Option<String>> {
    match settings::load(paths) {
        Ok(loaded) if section == "router" => Ok(loaded.router),
        Ok(loaded) => Ok(loaded.engine.program),
        Err(e) => Err(e),
    }
}

fn normalized(section: &str, raw: &str) -> String {
    if section == "router" {
        return Router::from(raw).name().to_string();
    }
    raw.trim().to_string()
}

fn move_file(paths: &RikuPaths, legacy: &LegacyFile) -> Result<Option<String>> {
    let raw = match fs::read_to_string(legacy.path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", legacy.path.display())),
    };
    let current = match current_value(paths, legacy.section) {
        Ok(current) => current,
        Err(e) => return Err(e),
    };
    let value = normalized(legacy.section, &raw);
    if current.is_none() && !value.is_empty() {
        if let Err(e) = settings::save_key(paths, legacy.section, legacy.key, Some(&value)) {
            return Err(e);
        }
    }
    if let Err(e) = fs::remove_file(legacy.path) {
        return Err(anyhow!("cannot remove {}: {e}", legacy.path.display()));
    }
    let file = legacy.path.display();
    Ok(Some(match current {
        Some(_) => format!(
            "removed {file}; settings.toml already sets {}.{}",
            legacy.section, legacy.key
        ),
        None => format!(
            "moved {file} into settings.toml {}.{}",
            legacy.section, legacy.key
        ),
    }))
}

fn migrate_server_files(paths: &RikuPaths) -> Vec<Check> {
    let legacy = [
        LegacyFile {
            path: &paths.router_file,
            section: "router",
            key: "name",
        },
        LegacyFile {
            path: &paths.container_engine_file,
            section: "engine",
            key: "program",
        },
    ];
    legacy
        .iter()
        .filter_map(|file| match move_file(paths, file) {
            Ok(Some(detail)) => Some(Check::ok(NAME, detail)),
            Ok(None) => None,
            Err(e) => Some(Check::fail(NAME, format!("{e:#}"))),
        })
        .collect()
}

fn app_names(paths: &RikuPaths) -> Vec<String> {
    let entries = match fs::read_dir(&paths.env_root) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .filter(|name| crate::util::validate_app_name(name).is_ok())
        .collect();
    names.sort();
    names
}

/// Run the migration without the root check (tests run unprivileged).
fn migrate(paths: &RikuPaths) -> Vec<Check> {
    let mut checks = migrate_server_files(paths);
    for app in app_names(paths) {
        checks.extend(migrate_env::migrate_app(paths, &app));
    }
    if checks.is_empty() {
        checks.push(Check::ok(NAME, "nothing to migrate"));
    }
    checks
}

/// Move legacy config into settings.toml and the per app overrides.
pub fn migrate_settings(paths: &RikuPaths) -> Vec<Check> {
    if nix::unistd::Uid::effective().is_root() {
        return vec![Check::warn(
            NAME,
            "skipped as root: run `riku doctor --fix` as the deploy user",
        )];
    }
    migrate(paths)
}

#[cfg(test)]
#[path = "migrate_settings_tests.rs"]
mod tests;
