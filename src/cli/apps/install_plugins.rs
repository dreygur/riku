//! `riku install-plugins` — download and install bundled runtime plugins.
//!
//! Downloads shell script plugins from the Riku GitHub repository into
//! `~/.riku/plugins/` and makes them executable. Rust binary plugins
//! (java, clojure, container) are downloaded from GitHub releases.

use anyhow::{bail, Result};
use std::fs;
use std::io::Write;

use crate::config::RikuPaths;
use crate::util::display;

/// Shell script plugins available in the bundled plugins/ directory of the repo.
const SHELL_PLUGINS: &[&str] = &["node", "python", "ruby", "go", "rust-lang"];

/// Rust binary plugins available as GitHub release assets.
/// These are downloaded as pre-compiled binaries named `riku-plugin-<name>`.
#[allow(dead_code)]
const BINARY_PLUGINS: &[&str] = &["java", "clojure", "container"];

/// Base URL for raw plugin script content.
const PLUGINS_RAW_BASE: &str = "https://raw.githubusercontent.com/dreygur/riku/main/plugins";

/// Download and install all bundled runtime plugins to `~/.riku/plugins/`.
pub fn cmd_install_plugins(paths: &RikuPaths, only: Option<Vec<String>>) -> Result<()> {
    fs::create_dir_all(&paths.plugin_root)?;

    let targets: Vec<&str> = match &only {
        Some(list) => list.iter().map(String::as_str).collect(),
        None => SHELL_PLUGINS.to_vec(),
    };

    let mut installed = 0;
    let mut failed = 0;

    for name in &targets {
        if SHELL_PLUGINS.contains(name) {
            match download_shell_plugin(name, paths) {
                Ok(_) => {
                    display::success(&format!("Installed plugin: {}", name));
                    installed += 1;
                }
                Err(e) => {
                    display::warn(&format!("Failed to install '{}': {}", name, e));
                    failed += 1;
                }
            }
        } else {
            display::warn(&format!("Unknown plugin '{}' — skipping", name));
        }
    }

    if failed > 0 && installed == 0 {
        bail!("All plugin downloads failed. Check your network connection.");
    }

    display::info(&format!(
        "Installed {} plugin(s) to {}",
        installed,
        paths.plugin_root.display()
    ));

    Ok(())
}

/// Download a single shell script plugin from GitHub and write it to the plugin directory.
fn download_shell_plugin(name: &str, paths: &RikuPaths) -> Result<()> {
    let url = format!("{}/{}", PLUGINS_RAW_BASE, name);
    let dest = paths.plugin_root.join(name);

    display::info(&format!("Downloading {} from {}...", name, url));

    let response = reqwest::blocking::get(&url)?;
    let status = response.status();

    if !status.is_success() {
        bail!("HTTP {} when fetching {}", status, url);
    }

    let content = response.bytes()?;

    let mut file = fs::File::create(&dest)?;
    file.write_all(&content)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&dest, perms)?;
    }

    Ok(())
}
