//! The `Runtime plugins` step of `riku init`: refresh the bundled plugins
//! this user has installed and not changed, and say what was left alone.
//! The work is [`crate::apps::plugin_refresh`]; this module only prints.

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

/// Refresh the plugins in `paths`' plugin directory. A failure is shown as a
/// warning rather than stopping init: the plugins already there still work,
/// and the rest of init (keys, service) matters more.
pub(super) fn refresh_runtime_plugins(paths: &RikuPaths) {
    match refresh_bundled(&paths.plugin_root) {
        Ok(report) => print_report(&report),
        Err(e) => {
            display::warn(&format!("{LABEL} not refreshed: {e:#}"));
        }
    }
}
