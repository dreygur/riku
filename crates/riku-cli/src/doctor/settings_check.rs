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

/// A warning when settings.toml has no `[routing]` section, so apps without
/// a domain get no URL. `None` when it is there or the file does not parse
/// (which [`settings_file`] reports).
pub fn routing_section(paths: &RikuPaths) -> Option<Check> {
    match settings::load(paths) {
        Ok(loaded) if !loaded.routing.present => Some(Check::warn(
            NAME,
            "no [routing] section, so apps without a domain get no URL; `riku init` adds it",
        )),
        _ => None,
    }
}

#[cfg(test)]
#[path = "settings_check_tests.rs"]
mod tests;
