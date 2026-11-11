//! The `Runtime plugins` step of `riku init`: refresh the bundled plugins
//! this user has installed and not changed, and say what was left alone.
//! The work is [`crate::apps::plugin_refresh`]; this module only prints.

use std::path::Path;

use crate::apps::install_plugins::BINARY_PLUGINS;
use crate::apps::plugin_refresh::{refresh_bundled, RefreshReport};
use crate::config::RikuPaths;
use crate::util::display;

const LABEL: &str = "Runtime plugins";

fn print_report(report: &RefreshReport) {
    if !report.refreshed.is_empty() {
        display::step_done(LABEL, &format!("refreshed {}", report.refreshed.join(", ")));
    }
    for name in &report.modified {
        display::warn(&format!(
            "{name} plugin was changed here, so it was not refreshed"
        ));
        display::detail(&format!(
            "To replace it with this riku's copy: riku plugins install {name}"
        ));
    }
    for (name, why) in &report.refused {
        display::warn(&format!("{name} plugin left alone: {why}"));
    }
    if !report.unrecorded.is_empty() {
        display::warn(&format!(
            "Not refreshed, installed by an older riku: {}",
            report.unrecorded.join(", ")
        ));
        display::detail(&format!(
            "To refresh them: riku install-plugins --plugins {}",
            report.unrecorded.join(",")
        ));
    }
}

/// The downloaded plugins (java, clojure) installed in `plugin_root`.
fn installed_downloads(plugin_root: &Path) -> Vec<&'static str> {
    BINARY_PLUGINS
        .iter()
        .copied()
        .filter(|name| plugin_root.join(name).is_file())
        .collect()
}

/// The warning and the reinstall command for `installed` downloaded
/// plugins, or `None` when there are none.
fn downloads_notice(installed: &[&str]) -> Option<(String, String)> {
    if installed.is_empty() {
        return None;
    }
    Some((
        format!(
            "Not refreshed, downloaded plugins: {}",
            installed.join(", ")
        ),
        format!(
            "To update them to this riku's release: riku install-plugins --plugins {}",
            installed.join(",")
        ),
    ))
}

/// Refresh the plugins in `paths`' plugin directory. A failure is shown as a
/// warning rather than stopping init: the plugins already there still work,
/// and the rest of init (keys, service) matters more.
///
/// Init never downloads, so java and clojure keep the release they were
/// installed from; the step only names them and the command to update them.
pub(super) fn refresh_runtime_plugins(paths: &RikuPaths) {
    match refresh_bundled(&paths.plugin_root) {
        Ok(report) => print_report(&report),
        Err(e) => {
            display::warn(&format!("{LABEL} not refreshed: {e:#}"));
        }
    }
    if let Some((warning, command)) = downloads_notice(&installed_downloads(&paths.plugin_root)) {
        display::warn(&warning);
        display::detail(&command);
    }
}

#[cfg(test)]
#[path = "runtime_plugins_tests.rs"]
mod tests;
