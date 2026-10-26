//! Deploy keys given to `riku init --ssh-key <file>`.
//!
//! A root init reads the file as root (it may sit in an admin's home the
//! deploy user cannot open), keeps only validated [`PublicKey`]s and passes
//! them to the deploy user's init as hidden `--add-key` arguments. Public
//! keys are not secret, so argv is fine. The deploy user's run writes them
//! to its own `authorized_keys`, each forced through the installed riku.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};

use crate::util::public_key::{parse_public_keys, PublicKey};
use crate::util::{echo, setup_authorized_keys};

use super::binary::get_user_install_directory;

/// The keys in a `.pub` or `authorized_keys` file.
pub fn read_key_file(path: &Path) -> Result<Vec<PublicKey>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => return Err(anyhow!("failed to read {}: {e}", path.display())),
    };
    match parse_public_keys(&text) {
        Ok(keys) => Ok(keys),
        Err(e) => Err(anyhow!("{}: {e}", path.display())),
    }
}

/// The keys a root init passed as `--add-key` arguments.
pub fn parse_key_args(lines: &[String]) -> Result<Vec<PublicKey>> {
    lines
        .iter()
        .map(|line| PublicKey::try_from(line.as_str()))
        .collect()
}

fn authorized_keys_text() -> Result<String> {
    let Some(home) = dirs::home_dir() else {
        return Err(anyhow!("could not determine the home directory"));
    };
    let path = home.join(".ssh").join("authorized_keys");
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(anyhow!("failed to read {}: {e}", path.display())),
    }
}

/// `ssh-keygen`'s fingerprint for `key`, which also proves the blob decodes.
fn fingerprint(key: &PublicKey) -> Result<String> {
    let mut child = match Command::new("ssh-keygen")
        .args(["-l", "-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => return Err(anyhow!("failed to run ssh-keygen: {e}")),
    };
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = writeln!(stdin, "{key}") {
            return Err(anyhow!("failed to feed ssh-keygen: {e}"));
        }
    }
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("failed to wait for ssh-keygen: {e}")),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    match stdout.split_whitespace().nth(1) {
        Some(fp) if output.status.success() => Ok(fp.to_string()),
        _ => Err(anyhow!(
            "ssh-keygen rejected the key: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

/// Add `keys` to this user's `authorized_keys`, skipping any already there.
#[allow(clippy::question_mark)]
pub fn add_deploy_keys(keys: &[PublicKey]) -> Result<()> {
    let binary = match get_user_install_directory() {
        Ok(dir) => dir.join("riku"),
        Err(e) => return Err(e),
    };
    let existing = match authorized_keys_text() {
        Ok(text) => text,
        Err(e) => return Err(e),
    };

    for key in keys {
        if existing.contains(key.blob()) {
            echo("      ✓ Key already authorized", "green");
            continue;
        }
        let fp = match fingerprint(key) {
            Ok(fp) => fp,
            Err(e) => return Err(e),
        };
        if let Err(e) = setup_authorized_keys(&fp, &binary.to_string_lossy(), &key.to_string()) {
            return Err(e);
        }
        echo(&format!("      ✓ Added deploy key {fp}"), "green");
    }
    Ok(())
}
