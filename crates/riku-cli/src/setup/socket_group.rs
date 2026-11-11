//! Root init's part of the uWSGI socket directory: the `riku-sockets` group,
//! `/run/riku-sockets` itself, and the systemd-tmpfiles entry that brings
//! the directory back after a reboot.
//!
//! The directory lives outside the deploy user's home, so the nginx worker
//! reaches app sockets through one group that grants nothing else. The
//! deploy home, `~/.riku` and the ENV files in it stay closed to nginx.
//!
//! Security: root writes only into root-owned paths here (`/run`,
//! `/etc/tmpfiles.d`), never into the deploy user's home. An account name
//! reaches `usermod` or the tmpfiles line only after it is checked to be a
//! plain account name, and commands run with a fixed argv and no shell.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Result};
use nix::unistd::Group;

use crate::config::{SOCKET_GROUP, SOCKET_ROOT};
use crate::util::socket_dir::{self, SOCKET_DIR_MODE};
use crate::util::{display, echo};

use super::caddy_host::run;
use super::deploy_user::DeployUser;
use super::nginx_host::{check_write_dir, ROOT_UID};

const TMPFILES_DIR: &str = "/etc/tmpfiles.d";
const TMPFILES_FILE: &str = "riku.conf";
const TMPFILES_PROGRAMS: [&str; 2] = ["/usr/bin/systemd-tmpfiles", "/bin/systemd-tmpfiles"];

/// Letters, digits, `_`, `-` and `.`, not starting with `-`: what useradd
/// accepts, and nothing a command line or a tmpfiles line could misread.
pub(super) fn is_account_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// The tmpfiles.d line that recreates the socket directory at boot.
pub(super) fn tmpfiles_line(user: &str) -> String {
    format!("d {SOCKET_ROOT} {SOCKET_DIR_MODE:04o} {user} {SOCKET_GROUP} -\n")
}

fn ensure_group() -> Result<()> {
    match Group::from_name(SOCKET_GROUP) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => run("groupadd", &["--system", SOCKET_GROUP]),
        Err(e) => Err(anyhow!("failed to look up group {SOCKET_GROUP}: {e}")),
    }
}

/// Add `account` to the socket group. The group must exist.
pub(super) fn add_to_socket_group(account: &str) -> Result<()> {
    if !is_account_name(account) {
        return Err(anyhow!(
            "refusing to add {account:?} to {SOCKET_GROUP}: not a plain account name"
        ));
    }
    run("usermod", &["-aG", SOCKET_GROUP, account])
}

fn has_tmpfiles() -> bool {
    TMPFILES_PROGRAMS
        .iter()
        .any(|program| Path::new(program).is_file())
}

/// Write `/etc/tmpfiles.d/riku.conf` unless it already holds `line`.
#[allow(clippy::question_mark)]
fn write_tmpfiles_entry(line: &str) -> Result<()> {
    let dir = Path::new(TMPFILES_DIR);
    if let Err(e) = fs::create_dir_all(dir) {
        return Err(anyhow!("failed to create {TMPFILES_DIR}: {e}"));
    }
    if let Err(e) = check_write_dir(dir, ROOT_UID) {
        return Err(e);
    }
    let file = dir.join(TMPFILES_FILE);
    match fs::read_to_string(&file) {
        Ok(current) if current == line => return Ok(()),
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(anyhow!("cannot read {}: {e}", file.display())),
    }
    match fs::write(&file, line) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to write {}: {e}", file.display())),
    }
}

fn keep_after_reboot(user: &str) -> Result<()> {
    if has_tmpfiles() {
        return write_tmpfiles_entry(&tmpfiles_line(user));
    }
    echo(
        &format!(
            "systemd-tmpfiles not found, so {SOCKET_ROOT} is gone after a reboot. \
             The supervisor recreates it only if it can write /run; otherwise re-run \
             sudo riku init after each boot"
        ),
        "yellow",
    );
    Ok(())
}

/// Create the socket group with the deploy user in it, the socket directory
/// owned by the deploy user, and its tmpfiles entry. Root only. Not unit
/// tested beyond its pure parts: it needs root and real accounts.
#[allow(clippy::question_mark)]
pub(super) fn setup_socket_root(user: &DeployUser) -> Result<()> {
    if !is_account_name(&user.name) {
        return Err(anyhow!(
            "deploy user {:?} is not a plain account name",
            user.name
        ));
    }
    if let Err(e) = ensure_group() {
        return Err(e);
    }
    if let Err(e) = add_to_socket_group(&user.name) {
        return Err(e);
    }
    if let Err(e) = socket_dir::prepare(Path::new(SOCKET_ROOT), Some(user.uid()), SOCKET_GROUP) {
        return Err(e);
    }
    if let Err(e) = keep_after_reboot(&user.name) {
        return Err(e);
    }
    display::step_done("Sockets", &format!("{SOCKET_ROOT}, group {SOCKET_GROUP}"));
    Ok(())
}

#[cfg(test)]
#[path = "socket_group_tests.rs"]
mod tests;
