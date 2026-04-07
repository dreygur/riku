/// Binary installation: install riku binary to user's PATH.
use anyhow::{bail, Result};
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::util::echo;

/// Install riku binary to user's PATH
pub fn install_riku_binary() -> Result<()> {
    let current_exe = env::current_exe()?;

    let target_dir = get_user_install_directory()?;
    let target_path = target_dir.join("riku");

    // Already installed in correct location
    if target_path.exists() && current_exe == target_path {
        return Ok(());
    }

    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)?;
    }

    echo(
        &format!("Installing riku to '{}'...", target_path.display()),
        "green",
    );
    fs::copy(&current_exe, &target_path)?;

    let mut perms = fs::metadata(&target_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&target_path, perms)?;

    echo(
        &format!("✓ Installed to {}", target_path.display()),
        "green",
    );

    if !is_in_path(&target_dir) {
        echo("", "");
        echo(
            &format!("⚠ Note: {} is not in your PATH.", target_dir.display()),
            "",
        );

        if !try_add_to_shell_config() {
            echo(
                "Add it manually to your shell config (~/.bashrc, ~/.zshrc):",
                "",
            );
            echo("  export PATH=\"$HOME/.local/bin:$PATH\"", "");
        }

        echo("", "");
        echo("Or reload your shell, or use the full path:", "");
        echo(&format!("  {}", target_path.display()), "");
        echo("", "");
        echo("After adding to PATH, run: exec $SHELL -l", "");
    }

    Ok(())
}

/// Attempt to add ~/.local/bin to shell config files; returns true if added.
fn try_add_to_shell_config() -> bool {
    let shell_configs = [".bashrc", ".zshrc", ".profile"];

    if let Ok(home) = env::var("HOME") {
        for config in &shell_configs {
            let config_path = PathBuf::from(&home).join(config);
            if config_path.exists() {
                if let Ok(content) = fs::read_to_string(&config_path) {
                    if !content.contains(".local/bin") {
                        if let Ok(mut file) =
                            fs::OpenOptions::new().append(true).open(&config_path)
                        {
                            let _ = writeln!(
                                file,
                                "\n# Add Riku to PATH\nexport PATH=\"$HOME/.local/bin:$PATH\""
                            );
                            echo(&format!("✓ Added PATH export to ~/{}", config), "");
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

/// Get user-local installation directory (~/.local/bin)
pub fn get_user_install_directory() -> Result<PathBuf> {
    if let Ok(home) = env::var("HOME") {
        return Ok(PathBuf::from(&home).join(".local/bin"));
    }

    if let Ok(cwd) = env::current_dir() {
        return Ok(cwd);
    }

    bail!("Could not determine home directory for installation")
}

/// Check if a directory is in the PATH environment variable
pub fn is_in_path(dir: &Path) -> bool {
    if let Ok(path) = env::var("PATH") {
        for path_dir in env::split_paths(&path) {
            if path_dir == dir {
                return true;
            }
        }
    }
    false
}
