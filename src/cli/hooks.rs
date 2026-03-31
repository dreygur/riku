//! Handler functions for the `riku hook` subcommand group.

use anyhow::Result;

use crate::config::RikuPaths;
use crate::plugins;
use crate::plugins::discovery;

/// List all executable server-side hook plugins installed in ~/.riku/plugins/.
pub fn cmd_hook_list(paths: &RikuPaths) -> Result<()> {
    let hooks = plugins::list_plugins(paths)?;
    if hooks.is_empty() {
        println!("No server-side hook plugins installed.");
        println!();
        println!("Install hook plugins by placing executable scripts in:");
        println!("  ~/.riku/plugins/");
        println!();
        println!("Supported hook names:");
        println!("  riku-pre-deploy   — runs before deploy, abort on failure");
        println!("  riku-pre-build    — runs before build, abort on failure");
        println!("  riku-post-build   — runs after build, failure is a warning");
        println!("  riku-post-deploy  — runs after deploy, failure is a warning");
    } else {
        for hook in hooks {
            println!("  {}", hook);
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
        println!("Hook plugin '{}' exists and is executable.", name);
        println!("  Path: {}", plugin_path.display());
        std::process::exit(0);
    } else {
        println!("Hook plugin '{}' not found or not executable.", name);
        std::process::exit(1);
    }
}
