//! `rk server add`: authorize this machine's public key on the new server.
//!
//! Only root on the server can add a deploy key, so the key goes through the
//! server's admin login: `ssh -t <admin> sudo riku deploy-key add --user <deploy user> '<key>'`.
//! Without an admin login the command is printed for the operator to run.
//!
//! Security model: the key file is read here and checked with
//! [`PublicKey`]; only its type and key data are sent, never the comment.
//! The remote command is built with `shlex::try_join`, so the key is one
//! argument to the remote shell whatever it holds. It goes on argv rather
//! than stdin, which stays free for the sudo password prompt.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Result};

use super::types::{Server, SshAddress};
use crate::util::public_key::{parse_public_keys, PublicKey};

const DEFAULT_KEYS: [&str; 2] = [".ssh/id_ed25519.pub", ".ssh/id_rsa.pub"];

/// The key to send: `explicit`, else the first default key under `home`
/// that exists. `None` when there is no key to send.
pub fn key_path(explicit: Option<&Path>, home: &Path) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return Some(path.to_path_buf());
    }
    DEFAULT_KEYS
        .iter()
        .map(|file| home.join(file))
        .find(|path| path.is_file())
}

/// The one public key in the file at `path`.
#[allow(clippy::question_mark)]
pub fn read_public_key(path: &Path) -> Result<PublicKey> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => return Err(anyhow!("failed to read {}: {e}", path.display())),
    };
    let mut keys = match parse_public_keys(&text) {
        Ok(keys) => keys,
        Err(e) => return Err(anyhow!("{}: {e}", path.display())),
    };
    if keys.len() != 1 {
        return Err(anyhow!(
            "{}: expected one public key, found {}",
            path.display(),
            keys.len()
        ));
    }
    Ok(keys.remove(0))
}

/// The server command. sudo drops `RIKU_USER`, so the deploy user is named
/// from the push address. `SshAddress` limits it to a safe label.
fn remote_command(address: &SshAddress, key_line: &str) -> Result<String> {
    let words = [
        "sudo",
        "riku",
        "deploy-key",
        "add",
        "--user",
        address.user(),
        key_line,
    ];
    match shlex::try_join(words) {
        Ok(command) => Ok(command),
        Err(e) => Err(anyhow!("cannot quote the key for ssh: {e}")),
    }
}

fn host_of(address: &SshAddress) -> &str {
    address
        .as_str()
        .split_once('@')
        .map_or("", |(_, host)| host)
}

/// What to tell the operator when the key was not sent. `key_line` is type
/// and base64 data only, so the single quotes are enough.
fn manual_hint(key_line: &str, server: &Server) -> String {
    let command = format!(
        "Add the key on the server: sudo riku deploy-key add --user {} '{key_line}'",
        server.address.user()
    );
    match server.admin {
        Some(_) => command,
        None => format!(
            "{command}\nor rerun with --admin <your login>@{}",
            host_of(&server.address)
        ),
    }
}

#[allow(clippy::question_mark)]
fn send_key(server: &Server, admin: &SshAddress, key_line: &str) -> Result<()> {
    let command = match remote_command(&server.address, key_line) {
        Ok(command) => command,
        Err(e) => return Err(e),
    };
    let status = Command::new("ssh")
        .args(["-t", "--", &admin.destination(), &command])
        .status();
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(anyhow!("ssh {} exited with {status}", admin.as_str())),
        Err(e) => Err(anyhow!("failed to start ssh: {e}")),
    }
}

/// Authorize `key` on `server` through its admin login, or print the
/// command to run there. A failed send prints the same command and is an
/// error; the server entry is kept either way.
pub fn install_key(server: &Server, key: &PublicKey) -> Result<()> {
    let key_line = key.to_string();
    let Some(admin) = &server.admin else {
        println!("{}", manual_hint(&key_line, server));
        return Ok(());
    };
    match send_key(server, admin, &key_line) {
        Ok(()) => Ok(()),
        Err(e) => {
            println!("{}", manual_hint(&key_line, server));
            Err(e)
        }
    }
}

#[cfg(test)]
#[path = "key_install_tests.rs"]
mod tests;
