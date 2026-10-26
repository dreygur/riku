//! `riku doctor` check for app ENV files that still set renamed keys.
//!
//! Reads `envs/<app>/ENV` for every app directory under `paths.env_root`.
//! Read-only like every other doctor check.

use std::collections::HashMap;
use std::path::Path;

use crate::config::RikuPaths;
use crate::util::legacy_env::legacy_keys;

use super::Check;

const NAME: &str = "app env";

fn app_warnings(app_dir: &Path) -> Vec<Check> {
    let app = app_dir.file_name().unwrap_or_default().to_string_lossy();
    let mut env = HashMap::new();
    if let Err(e) = crate::util::parse_settings(&app_dir.join("ENV"), &mut env) {
        return vec![Check::warn(NAME, format!("{app}: could not read ENV: {e}"))];
    }

    legacy_keys(&env)
        .into_iter()
        .map(|(old, new)| Check::warn(NAME, format!("{app}: {old} is no longer read, use {new}")))
        .collect()
}

/// One warning per app and renamed key, or a single `Ok` when none is found.
pub fn renamed_keys(paths: &RikuPaths) -> Vec<Check> {
    let entries = match std::fs::read_dir(&paths.env_root) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return vec![Check::ok(NAME, "no app envs")];
        }
        Err(e) => {
            return vec![Check::warn(
                NAME,
                format!("could not read {}: {e}", paths.env_root.display()),
            )];
        }
    };

    let mut app_dirs: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    app_dirs.sort();

    let warnings: Vec<Check> = app_dirs.iter().flat_map(|dir| app_warnings(dir)).collect();
    if warnings.is_empty() {
        return vec![Check::ok(NAME, "no renamed keys in use")];
    }
    warnings
}

#[cfg(test)]
#[path = "app_env_tests.rs"]
mod tests;
