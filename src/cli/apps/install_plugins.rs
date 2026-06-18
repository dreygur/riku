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
/// These are downloaded as pre-compiled binaries named `riku-plugin-<name>-<target-triple>`
/// (see `.github/workflows/release.yml`, which builds and uploads them
/// alongside the main `riku` binary for every release).
const BINARY_PLUGINS: &[&str] = &["java", "clojure", "container"];

/// Base URL for raw plugin script content.
const PLUGINS_RAW_BASE: &str = "https://raw.githubusercontent.com/dreygur/riku/main/plugins";

/// Base URL for the latest GitHub release's downloadable assets.
const RELEASE_DOWNLOAD_BASE: &str = "https://github.com/dreygur/riku/releases/latest/download";

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
        let result = if SHELL_PLUGINS.contains(name) {
            download_shell_plugin(name, paths)
        } else if BINARY_PLUGINS.contains(name) {
            download_binary_plugin(name, paths)
        } else {
            display::warn(&format!("Unknown plugin '{}' — skipping", name));
            continue;
        };

        match result {
            Ok(_) => {
                display::success(&format!("Installed plugin: {}", name));
                installed += 1;
            }
            Err(e) => {
                display::warn(&format!("Failed to install '{}': {}", name, e));
                failed += 1;
            }
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

/// Map the running host to one of the release target triples built by
/// `.github/workflows/release.yml`. Returns an error for any host this repo
/// doesn't cross-compile binary plugins for.
fn host_target_triple() -> Result<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-gnu"),
        ("linux", "arm") => Ok("armv7-unknown-linux-gnueabihf"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        (os, arch) => bail!(
            "no pre-built '{}' binary plugin for {}/{} — see .github/workflows/release.yml for supported targets",
            os,
            os,
            arch
        ),
    }
}

/// Download a single pre-compiled Rust binary plugin from the latest GitHub
/// release and write it to the plugin directory under its bare name (e.g.
/// `java`, not `riku-plugin-java`), matching the runtime plugin discovery
/// protocol used by shell plugins.
fn download_binary_plugin(name: &str, paths: &RikuPaths) -> Result<()> {
    let target = host_target_triple()?;
    let url = format!("{}/riku-plugin-{}-{}", RELEASE_DOWNLOAD_BASE, name, target);
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

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&dest, perms)?;
    }

    Ok(())
}
