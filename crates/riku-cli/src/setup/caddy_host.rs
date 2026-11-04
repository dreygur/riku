//! Root-only host setup for the Caddy router.
//!
//! Caddy runs as the `caddy` user and needs exactly one thing from riku: to
//! read the per app site files. It proxies to TCP backends only, gets its
//! certificates into its own storage, and serves no files from disk. So the
//! site files live outside the deploy user's home, in `/etc/riku/caddy`,
//! owned by the deploy user, group `caddy`'s primary group, mode `2750`.
//! The deploy user writes them, `caddy` reads them, and `caddy` is put in
//! no group and given nothing in the deploy home. Setup also adds one
//! `import` line to the system Caddyfile.
//!
//! Security: every command runs with a fixed argv, no shell. Root writes
//! only system paths: `/etc/caddy/Caddyfile`, and `/etc/riku` and the site
//! directory in it. `/etc/riku` must be a root-owned directory no one else
//! can write, and the site directory is refused when it is a symlink, so a
//! planted link cannot redirect root's chown or chmod. The import line is
//! built from a constant path; no app value reaches the Caddyfile.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};
use nix::unistd::{Group, User};

use crate::caddy::{has_import, import_line, SYSTEM_CADDYFILE as CADDYFILE};
use crate::config::RikuPaths;
use crate::util::socket_dir;
use crate::util::{echo, write_atomic};

use super::deploy_user::DeployUser;
use super::nginx_host::{check_write_dir, ROOT_UID};

const CADDY_USER: &str = "caddy";

/// `current` with `line` appended on its own line, or `None` when that
/// exact line is already there.
fn caddyfile_with_import(current: &str, line: &str) -> Option<String> {
    if has_import(current, line) {
        return None;
    }
    let mut updated = current.to_string();
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(line);
    updated.push('\n');
    Some(updated)
}

/// Run `program` with a fixed argv and no shell.
pub(super) fn run(program: &str, args: &[&str]) -> Result<()> {
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
fn add_caddyfile_import(paths: &RikuPaths) -> Result<()> {
    let current = match fs::read_to_string(CADDYFILE) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(anyhow!(
                "{CADDYFILE} not found. Install the caddy package first"
            ))
        }
        Err(e) => return Err(anyhow!("failed to read {CADDYFILE}: {e}")),
    };

    let Some(updated) = caddyfile_with_import(&current, &import_line(paths)) else {
        return Ok(());
    };
    if let Err(e) = write_atomic(Path::new(CADDYFILE), updated.as_bytes()) {
        return Err(e);
    }
    echo(&format!("      ✓ {CADDYFILE} imports riku sites"), "green");
    Ok(())
}

/// The name of the `caddy` user's primary group, the only group the site
/// directory is shared with.
fn caddy_group() -> Result<String> {
    let caddy = match User::from_name(CADDY_USER) {
        Ok(Some(caddy)) => caddy,
        Ok(None) => {
            return Err(anyhow!(
                "user {CADDY_USER} not found. Install the caddy package first"
            ))
        }
        Err(e) => return Err(anyhow!("failed to look up user {CADDY_USER}: {e}")),
    };
    if caddy.gid.as_raw() == 0 {
        return Err(anyhow!(
            "{CADDY_USER}'s primary group is gid 0; riku will not share its site files with it"
        ));
    }
    match Group::from_gid(caddy.gid) {
        Ok(Some(group)) => Ok(group.name),
        Ok(None) => Err(anyhow!("no group with gid {}", caddy.gid)),
        Err(e) => Err(anyhow!("failed to look up gid {}: {e}", caddy.gid)),
    }
}

/// Create `site_root`'s parent if missing and require it to be owned by
/// `parent_owner` and writable by no one else, then make `site_root` itself
/// `2750`, owned by `owner` with group `group`.
#[allow(clippy::question_mark)]
fn prepare_site_root(
    site_root: &Path,
    parent_owner: u32,
    owner: Option<u32>,
    group: &str,
) -> Result<()> {
    let Some(parent) = site_root.parent() else {
        return Err(anyhow!("{} has no parent directory", site_root.display()));
    };
    if let Err(e) = fs::create_dir_all(parent) {
        return Err(anyhow!("failed to create {}: {e}", parent.display()));
    }
    if let Err(e) = check_write_dir(parent, parent_owner) {
        return Err(e);
    }
    socket_dir::prepare(site_root, owner, group)
}

/// Prepare this host to serve riku apps through Caddy. Root only.
///
/// The user and group lookups and the chown to real accounts need root, so
/// beyond [`prepare_site_root`] this is covered only by running init on a
/// host.
#[allow(clippy::question_mark)]
pub fn setup_caddy_host(paths: &RikuPaths, user: &DeployUser) -> Result<()> {
    if let Err(e) = run("caddy", &["version"]) {
        return Err(anyhow!("caddy is not installed or not on PATH: {e}"));
    }
    let group = match caddy_group() {
        Ok(group) => group,
        Err(e) => return Err(e),
    };
    if let Err(e) = prepare_site_root(&paths.caddy_root, ROOT_UID, Some(user.uid()), &group) {
        return Err(e);
    }
    if let Err(e) = add_caddyfile_import(paths) {
        return Err(e);
    }
    echo(
        &format!(
            "      ✓ {} ready for Caddy ({group} reads it)",
            paths.caddy_root.display()
        ),
        "green",
    );
    Ok(())
}

#[cfg(test)]
#[path = "caddy_host_tests.rs"]
mod tests;
