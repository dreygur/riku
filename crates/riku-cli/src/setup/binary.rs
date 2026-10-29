/// Binary installation: install riku binary to user's PATH.
use anyhow::{anyhow, bail, Result};
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::util::echo;

/// Where riku installs itself, relative to the user's home. The git hook and
/// the systemd unit run the binary from here.
pub const INSTALL_DIR: &str = ".local/bin";
/// The new binary's name in `INSTALL_DIR` until it is renamed into place.
const STAGED_NAME: &str = ".riku.new";
const BINARY_MODE: u32 = 0o755;

/// Swap `target` for a copy of `source` by rename, never writing into
/// `target`: a running supervisor holds it open, and Linux refuses writes
/// to a busy executable (ETXTBSY). The old inode lives on until it exits.
fn replace_binary(source: &Path, target: &Path) -> Result<()> {
    let staged = target.with_file_name(STAGED_NAME);
    if let Err(e) = fs::copy(source, &staged) {
        return Err(anyhow!("failed to copy riku to {}: {e}", staged.display()));
    }
    if let Err(e) = fs::set_permissions(&staged, fs::Permissions::from_mode(BINARY_MODE)) {
        return Err(anyhow!("failed to chmod {}: {e}", staged.display()));
    }
    if let Err(e) = fs::rename(&staged, target) {
        return Err(anyhow!(
            "failed to move riku into {}: {e}",
            target.display()
        ));
    }
    Ok(())
}

/// Whether install explains a missing PATH entry. A root init runs the
/// deploy user's steps, and the note is for a person at that user's shell.
pub enum PathNote {
    Show,
    Skip,
}

/// Install riku binary to user's PATH
#[allow(clippy::question_mark)]
pub fn install_riku_binary(note: PathNote) -> Result<()> {
    let current_exe = match env::current_exe() {
        Ok(exe) => exe,
        Err(e) => return Err(anyhow!("failed to find the running riku binary: {e}")),
    };

    let target_dir = match get_user_install_directory() {
        Ok(dir) => dir,
        Err(e) => return Err(e),
    };
    let target_path = target_dir.join("riku");

    // Already installed in correct location
    if target_path.exists() && current_exe == target_path {
        return Ok(());
    }

    if !target_dir.exists() {
        if let Err(e) = fs::create_dir_all(&target_dir) {
            return Err(anyhow!("failed to create {}: {e}", target_dir.display()));
        }
    }

    echo(
        &format!("Installing riku to '{}'...", target_path.display()),
        "green",
    );
    if let Err(e) = replace_binary(&current_exe, &target_path) {
        return Err(e);
    }

    echo(
        &format!("✓ Installed to {}", target_path.display()),
        "green",
    );

    if !is_in_path(&target_dir) {
        let added = try_add_to_shell_config();
        if matches!(note, PathNote::Show) {
            print_path_note(&target_dir, &target_path, added);
        }
    }

    Ok(())
}

fn print_path_note(target_dir: &Path, target_path: &Path, added: bool) {
    echo("", "");
    echo(
        &format!("Note: {} is not in your PATH.", target_dir.display()),
        "",
    );

    if !added {
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

/// Attempt to add ~/.local/bin to shell config files; returns true if added.
///
/// Writes to `.profile` (sourced for login shells and SSH sessions) and
/// `.bashrc`/`.zshrc` (sourced for interactive terminals) so `riku` is on
/// PATH regardless of how the shell is started.
fn try_add_to_shell_config() -> bool {
    let line = "\n# Add Riku to PATH\nexport PATH=\"$HOME/.local/bin:$PATH\"\n";
    let mut added = false;

    if let Ok(home) = env::var("HOME") {
        // .profile: sourced for login shells and non-interactive SSH
        // command execution. This is the critical one for remote usage.
        let profile = PathBuf::from(&home).join(".profile");
        added |= append_once(&profile, line, true);

        // .bashrc / .zshrc: sourced for interactive terminal sessions.
        for config in &[".bashrc", ".zshrc"] {
            let config_path = PathBuf::from(&home).join(config);
            append_once(&config_path, line, false);
        }
    }

    if added {
        echo("✓ Added PATH export to ~/.profile", "");
    }

    added
}

/// Append `line` to `path` unless it already contains `.local/bin`.
/// If `create` is false, does nothing when `path` doesn't exist.
/// Returns true if the file was written to.
fn append_once(path: &Path, line: &str, create: bool) -> bool {
    if !path.exists() && !create {
        return false;
    }
    if let Ok(content) = fs::read_to_string(path) {
        if content.contains(".local/bin") {
            return false;
        }
    }
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(create)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{}", line);
        return true;
    }
    false
}

/// Get user-local installation directory (~/.local/bin)
pub fn get_user_install_directory() -> Result<PathBuf> {
    if let Ok(home) = env::var("HOME") {
        return Ok(PathBuf::from(&home).join(INSTALL_DIR));
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

#[cfg(test)]
#[path = "binary_tests.rs"]
mod tests;
