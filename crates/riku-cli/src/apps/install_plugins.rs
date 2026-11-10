//! `riku install-plugins` and `riku plugins install <runtime>`: install the
//! runtime plugins riku ships.
//!
//! The five shell runtimes and the `riku-notify` bundle are built into riku
//! (`embedded_plugins`) and written out; java and clojure are downloaded from
//! this riku's release and checked (`plugin_download`). The supervisor's
//! `/control/plugins/install` route uses the same function.

use std::fs;

use anyhow::{anyhow, bail, Result};

use crate::config::RikuPaths;
use crate::util::display;

use super::embedded_plugins::{self, EMBEDDED_RUNTIMES};
use super::plugin_download::install_binary_plugin;

/// Runtime plugins riku downloads instead of carrying: pre-built binaries
/// published per target with every release (`.github/workflows/release.yml`).
pub(crate) const BINARY_PLUGINS: &[&str] = &["java", "clojure"];

/// Install one plugin by name: an embedded one is written out, java or
/// clojure is downloaded and verified. An unknown name is an error.
pub(crate) fn install_runtime(paths: &RikuPaths, name: &str) -> Result<()> {
    if let Some(plugin) = embedded_plugins::find(name) {
        return embedded_plugins::write_out(plugin, &paths.plugin_root);
    }
    if BINARY_PLUGINS.contains(&name) {
        return install_binary_plugin(paths, name);
    }
    bail!(
        "unknown plugin '{name}'; riku ships: {}, {}, riku-notify",
        EMBEDDED_RUNTIMES.join(", "),
        BINARY_PLUGINS.join(", ")
    )
}

/// Install `only`, or the five embedded runtimes when `None`. Fails, naming
/// every plugin that could not be installed, when any of them failed.
#[allow(clippy::question_mark)]
pub fn cmd_install_plugins(paths: &RikuPaths, only: Option<Vec<String>>) -> Result<()> {
    if let Err(e) = fs::create_dir_all(&paths.plugin_root) {
        return Err(anyhow!(
            "cannot create {}: {e}",
            paths.plugin_root.display()
        ));
    }
    let targets: Vec<String> = match only {
        Some(list) => list,
        None => EMBEDDED_RUNTIMES
            .iter()
            .map(|name| name.to_string())
            .collect(),
    };

    let mut failures = Vec::new();
    for name in &targets {
        match install_runtime(paths, name) {
            Ok(()) => display::success(&format!("Installed plugin: {name}")),
            Err(e) => failures.push(format!("{name}: {e:#}")),
        }
    }
    if !failures.is_empty() {
        bail!("could not install {}", failures.join("; "));
    }
    display::info(&format!(
        "Installed {} plugin(s) to {}",
        targets.len(),
        paths.plugin_root.display()
    ));
    Ok(())
}
