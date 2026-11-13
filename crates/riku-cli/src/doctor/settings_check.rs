//! `riku doctor` checks for `<riku root>/settings.toml`.
//!
//! Read-only like every other doctor check: it parses the file and reports
//! what the loader would refuse, naming the offending key, and which routing
//! applies to an app without a domain.

use crate::config::settings::{self, RoutingSettings};
use crate::config::RikuPaths;

use super::Check;

const NAME: &str = "settings";
const ROUTING: &str = "routing";

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

/// How an app without a domain of its own is reached under `routing`.
fn routing_detail(routing: &RoutingSettings) -> String {
    let defaults = if routing.present { "" } else { "defaults, " };
    match (&routing.fallback_domain, routing.server_ip) {
        (Some(pattern), _) if !pattern.needs_ip() => {
            format!("{defaults}fallback domain {}", pattern.as_str())
        }
        (Some(pattern), Some(ip)) => {
            format!("{defaults}fallback domain {} with {ip}", pattern.as_str())
        }
        (Some(pattern), None) => format!(
            "{defaults}fallback domain {} off (no server_ip), port mode",
            pattern.as_str()
        ),
        (None, _) => format!("{defaults}port mode, no fallback domain"),
    }
}

/// An ok line saying which routing applies to an app without a domain.
/// `None` when the file does not parse, which [`settings_file`] reports.
pub fn routing(paths: &RikuPaths) -> Option<Check> {
    match settings::load(paths) {
        Ok(loaded) => Some(Check::ok(ROUTING, routing_detail(&loaded.routing))),
        Err(_) => None,
    }
}

#[cfg(test)]
#[path = "settings_check_tests.rs"]
mod tests;
