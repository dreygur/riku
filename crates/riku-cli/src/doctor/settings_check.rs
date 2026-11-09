//! `riku doctor` check for `<riku root>/settings.toml`.
//!
//! Read-only like every other doctor check: it parses the file and reports
//! what the loader would refuse, naming the offending key.

use crate::config::settings;
use crate::config::RikuPaths;

use super::Check;

const NAME: &str = "settings";

/// Fail when the settings file cannot be read or holds a bad key or value,
/// ok otherwise.
pub fn settings_file(paths: &RikuPaths) -> Check {
    if !paths.settings_file.exists() {
        return Check::ok(NAME, "defaults");
    }
    match settings::load(paths) {
        Ok(_) => Check::ok(NAME, paths.settings_file.display().to_string()),
        Err(e) => Check::fail(NAME, e.to_string()),
    }
}

/// One warning per legacy single value file still present (`ROUTER`,
/// `CONTAINER_ENGINE`); they are still read, but settings.toml replaces them.
pub fn legacy_files(paths: &RikuPaths) -> Vec<Check> {
    [&paths.router_file, &paths.container_engine_file]
        .into_iter()
        .filter(|file| file.exists())
        .map(|file| {
            Check::warn(
                NAME,
                format!(
                    "{} is a legacy file; `riku doctor --fix` moves it into settings.toml",
                    file.display()
                ),
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "settings_check_tests.rs"]
mod tests;
