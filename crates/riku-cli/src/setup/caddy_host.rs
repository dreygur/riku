//! Root-only host setup for the Caddy router.
//!
//! Caddy runs as the `caddy` user and reads site files from the deploy
//! user's `~/.riku/caddy`. Setup adds one `import` line to the system
//! Caddyfile, puts `caddy` in the deploy user's primary group, gives that
//! group traverse (not list) access to the home and riku root, and creates
//! the site directory as `0750` owned by the deploy user.
//!
//! Security: every command runs with a fixed argv, no shell. The import line
//! is built from the riku root path only; no app value reaches the
//! Caddyfile.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::util::{echo, write_atomic};

use super::deploy_user::DeployUser;

const CADDYFILE: &str = "/etc/caddy/Caddyfile";
const CADDY_USER: &str = "caddy";
const SITE_DIR_MODE: u32 = 0o750;

fn import_line(caddy_root: &Path) -> String {
    format!("import {}/*.caddy", caddy_root.display())
}

/// `current` with `import_line` appended on its own line, or `None` when
/// that exact line is already there.
fn caddyfile_with_import(current: &str, import_line: &str) -> Option<String> {
    if current.lines().any(|line| line.trim() == import_line) {
        return None;
    }
    let mut updated = current.to_string();
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(import_line);
    updated.push('\n');
    Some(updated)
}

fn run(program: &str, args: &[&str]) -> Result<()> {
    let command = format!("{program} {}", args.join(" "));
    match Command::new(program).args(args).output() {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(anyhow!(
            "{command} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(e) => Err(anyhow!("failed to run {command}: {e}")),
    }
}

#[allow(clippy::question_mark)]
fn add_caddyfile_import(caddy_root: &Path) -> Result<()> {
    let current = match fs::read_to_string(CADDYFILE) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(anyhow!(
                "{CADDYFILE} not found. Install the caddy package first"
            ))
        }
        Err(e) => return Err(anyhow!("failed to read {CADDYFILE}: {e}")),
    };

    let Some(updated) = caddyfile_with_import(&current, &import_line(caddy_root)) else {
        return Ok(());
    };
    if let Err(e) = write_atomic(Path::new(CADDYFILE), updated.as_bytes()) {
        return Err(e);
    }
    echo(&format!("      ✓ {CADDYFILE} imports riku sites"), "green");
    Ok(())
}

#[allow(clippy::question_mark)]
fn grant_caddy_traverse(user: &DeployUser, paths: &RikuPaths) -> Result<()> {
    if let Err(e) = run("usermod", &["-aG", &user.group, CADDY_USER]) {
        return Err(e);
    }
    for dir in [&user.home, &paths.riku_root] {
        if let Err(e) = run("chmod", &["g+x", &dir.to_string_lossy()]) {
            return Err(e);
        }
    }
    Ok(())
}

#[allow(clippy::question_mark)]
fn create_site_dir(user: &DeployUser, caddy_root: &Path) -> Result<()> {
    if let Err(e) = fs::create_dir_all(caddy_root) {
        return Err(anyhow!("failed to create {}: {e}", caddy_root.display()));
    }
    let mode = fs::Permissions::from_mode(SITE_DIR_MODE);
    if let Err(e) = fs::set_permissions(caddy_root, mode) {
        return Err(anyhow!("failed to chmod {}: {e}", caddy_root.display()));
    }
    user.chown(caddy_root)
}

/// Prepare this host to serve riku apps through Caddy. Root only.
///
/// `paths` are the deploy user's riku paths, whose root must already exist.
/// The usermod, chmod and chown calls are not unit tested: they need root
/// and real users, so they are covered only by running init on a host.
#[allow(clippy::question_mark)]
pub fn setup_caddy_host(paths: &RikuPaths, user: &DeployUser) -> Result<()> {
    if let Err(e) = run("caddy", &["version"]) {
        return Err(anyhow!("caddy is not installed or not on PATH: {e}"));
    }
    if let Err(e) = add_caddyfile_import(&paths.caddy_root) {
        return Err(e);
    }
    if let Err(e) = grant_caddy_traverse(user, paths) {
        return Err(e);
    }
    if let Err(e) = create_site_dir(user, &paths.caddy_root) {
        return Err(e);
    }
    echo(
        &format!("      ✓ {} ready for Caddy", paths.caddy_root.display()),
        "green",
    );
    Ok(())
}

#[cfg(test)]
#[path = "caddy_host_tests.rs"]
mod tests;
