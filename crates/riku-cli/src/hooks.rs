//! Handler functions for the `riku hook` subcommand group.

use anyhow::Result;

use crate::apps::info::field;
use crate::config::RikuPaths;
use crate::util::display;
use riku_plugins as plugins;
use riku_plugins::discovery;

/// List all executable server-side hook plugins installed in ~/.riku/plugins/.
#[allow(clippy::question_mark)]
pub fn cmd_hook_list(paths: &RikuPaths) -> Result<()> {
    let hooks = match plugins::list_plugins(paths) {
        Ok(hooks) => hooks,
        Err(e) => return Err(e),
    };
    if hooks.is_empty() {
        display::warn("No server-side hook plugins installed.");
        display::detail("");
        display::detail("Install hook plugins by placing executable scripts in:");
        display::detail("  ~/.riku/plugins/");
        display::detail("");
        display::detail("Supported hook names:");
        display::detail("  riku-pre-deploy, runs before deploy, abort on failure");
        display::detail("  riku-pre-build, runs before build, abort on failure");
        display::detail("  riku-post-build, runs after build, failure is a warning");
        display::detail("  riku-post-deploy, runs after deploy, failure is a warning");
    } else {
        display::step_done("Installed Hook Plugins", "");
        for hook in hooks {
            display::detail(&format!("  {}", hook));
        }
    }
    Ok(())
}

/// Check if a named server-side hook plugin exists and is executable.
///
/// Prints the path on success (exits 0) or an error message (exits 1).
pub fn cmd_hook_check(paths: &RikuPaths, name: &str) -> Result<()> {
    if let Err(e) = discovery::validate_plugin_name(name) {
        return Err(anyhow::anyhow!("{}", e));
    }
    if plugins::plugin_exists(name, paths) {
        let plugin_path = paths.plugin_root.join(name);
        display::step_done(
            &format!("Hook plugin '{}' exists and is executable.", name),
            "",
        );
        field("Path:", &plugin_path.display().to_string());
        std::process::exit(0);
    } else {
        display::warn(&format!(
            "Hook plugin '{}' not found or not executable.",
            name
        ));
        std::process::exit(1);
    }
}
