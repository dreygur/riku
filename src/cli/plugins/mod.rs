//! `riku plugins` provider layer — install/list/remove manifest-based plugin
//! bundles via [`PluginInstaller`]. Handles user-facing output only.

use anyhow::Result;

use crate::config::RikuPaths;
use crate::plugins::PluginInstaller;
use crate::util::display;

/// `riku plugins install <source>`
pub fn cmd_plugins_install(paths: &RikuPaths, source: &str) -> Result<()> {
    display::info(&format!("Installing plugin from {source}..."));
    let manifest = PluginInstaller::new(paths).install(source)?;
    display::success(&format!(
        "Installed {} v{} ({:?})",
        manifest.name, manifest.version, manifest.plugin_type
    ));
    if manifest.checksum.is_some() {
        display::note("Checksum verified.");
    } else {
        display::warn("No checksum pinned in the manifest — installed unverified.");
    }
    Ok(())
}

/// `riku plugins list`
pub fn cmd_plugins_list(paths: &RikuPaths) -> Result<()> {
    let installed = PluginInstaller::new(paths).list();
    if installed.is_empty() {
        display::note("No plugin bundles installed. Add one: riku plugins install <source>");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = installed
        .iter()
        .map(|(manifest, lock)| {
            let verified = if lock.as_ref().is_some_and(|l| l.author_pinned) {
                "yes"
            } else {
                "no"
            };
            vec![
                manifest.name.clone(),
                manifest.version.clone(),
                format!("{:?}", manifest.plugin_type).to_lowercase(),
                verified.to_string(),
            ]
        })
        .collect();
    display::print_table(&["NAME", "VERSION", "TYPE", "VERIFIED"], &rows, 2);
    Ok(())
}

/// `riku plugins remove <name>`
pub fn cmd_plugins_remove(paths: &RikuPaths, name: &str) -> Result<()> {
    PluginInstaller::new(paths).remove(name)?;
    display::success(&format!("Removed plugin '{name}'."));
    Ok(())
}
