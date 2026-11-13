//! `riku plugin-emit`: lets a plugin's own script fire a custom
//! `plugin.custom.*` event (`PLUGIN_PROTOCOL.md` §7.3).
//!
//! Must be run from within a plugin invocation (reads `RIKU_PLUGIN_NAME`,
//! set by every dispatch site alongside `RIKU_PLUGIN_API`/`RIKU_ROOT`), and
//! that plugin must have declared `[events] emit = true` in its own
//! manifest. The `plugin.custom.` namespace is enforced here, not just
//! documented: a plugin can never emit something that looks like a kernel
//! event (`app.restarted`, `deploy.finished`, …), which is what keeps those
//! events trustworthy for subscribers.

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::util::display;
use riku_plugins::{EventBus, PluginManifest};

const CUSTOM_NAMESPACE: &str = "plugin.custom.";

pub fn cmd_plugin_emit(
    paths: &RikuPaths,
    event_name: &str,
    data: &str,
    app: Option<&str>,
) -> Result<()> {
    let plugin_name = std::env::var("RIKU_PLUGIN_NAME").ok();
    emit_as(paths, plugin_name, event_name, data, app)
}

/// [`cmd_plugin_emit`] for the plugin `plugin_name`, the `RIKU_PLUGIN_NAME`
/// value, so tests pass it instead of setting the variable.
fn emit_as(
    paths: &RikuPaths,
    plugin_name: Option<String>,
    event_name: &str,
    data: &str,
    app: Option<&str>,
) -> Result<()> {
    let plugin_name = match plugin_name {
        Some(name) => name,
        None => {
            return Err(anyhow!(
                "riku plugin-emit must be run from within a plugin invocation \
                 (RIKU_PLUGIN_NAME is unset: this isn't meant to be run by hand)"
            ))
        }
    };

    if !event_name.starts_with(CUSTOM_NAMESPACE) {
        return Err(anyhow!(
            "custom event names must start with '{CUSTOM_NAMESPACE}' (got '{event_name}'), \
             a plugin can never emit something that looks like a kernel event"
        ));
    }

    let manifest = match PluginManifest::from_dir(&paths.plugin_root.join(&plugin_name)) {
        Ok(manifest) => manifest,
        Err(e) => {
            return Err(anyhow!(
                "loading manifest for plugin '{plugin_name}': {e:#}"
            ))
        }
    };

    if !manifest.events.emit {
        return Err(anyhow!(
            "plugin '{plugin_name}' has not declared `[events] emit = true` in its manifest, \
             add it before calling `riku plugin-emit`"
        ));
    }

    let value: serde_json::Value = match serde_json::from_str(data) {
        Ok(value) => value,
        Err(e) => return Err(anyhow!("--data is not valid JSON: {data}: {e}")),
    };

    EventBus::new(paths).publish_custom(event_name, &plugin_name, app.unwrap_or(""), value);
    display::step_done(&format!("emitted '{event_name}' from '{plugin_name}'"), "");
    Ok(())
}

#[cfg(test)]
#[path = "plugin_emit_tests.rs"]
mod tests;
