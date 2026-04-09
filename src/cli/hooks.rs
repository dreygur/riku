//! Handler functions for the `riku hook` subcommand group.

use anyhow::Result;

use crate::config::RikuPaths;
use crate::plugins;
use crate::plugins::discovery;
use crate::util::display;

/// List all executable server-side hook plugins installed in ~/.riku/plugins/.
pub fn cmd_hook_list(paths: &RikuPaths) -> Result<()> {
    let hooks = plugins::list_plugins(paths)?;
    if hooks.is_empty() {
        display::warn("No server-side hook plugins installed.");
        display::blank();
        display::note("Install hook plugins by placing executable scripts in:");
        display::note("  ~/.riku/plugins/");
        display::blank();
        display::note("Supported hook names:");
        display::note("  riku-pre-deploy   — runs before deploy, abort on failure");
        display::note("  riku-pre-build    — runs before build, abort on failure");
        display::note("  riku-post-build   — runs after build, failure is a warning");
        display::note("  riku-post-deploy  — runs after deploy, failure is a warning");
    } else {
        display::section("Installed Hook Plugins");
        for hook in hooks {
            display::note(&format!("  {}", hook));
        }
    }
    Ok(())
}

/// Check if a named server-side hook plugin exists and is executable.
///
/// Prints the path on success (exits 0) or an error message (exits 1).
pub fn cmd_hook_check(paths: &RikuPaths, name: &str) -> Result<()> {
    discovery::validate_plugin_name(name).map_err(|e| anyhow::anyhow!("{}", e))?;
    if plugins::plugin_exists(name, paths) {
        let plugin_path = paths.plugin_root.join(name);
        display::success(&format!("Hook plugin '{}' exists and is executable.", name));
        display::kv("Path:", &plugin_path.display().to_string());
        std::process::exit(0);
    } else {
        display::warn(&format!(
            "Hook plugin '{}' not found or not executable.",
            name
        ));
        std::process::exit(1);
    }
}
