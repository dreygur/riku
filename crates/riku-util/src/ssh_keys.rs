//! SSH authorized_keys management utilities.

use anyhow::{anyhow, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::authorized_keys::authorized_keys_path;

/// Append to ~/.ssh/authorized_keys with SSH restrictions.
/// Set directory permissions to 700, file permissions to 600.
pub fn setup_authorized_keys(ssh_fingerprint: &str, script_path: &str, pubkey: &str) -> Result<()> {
    match std::env::var("HOME") {
        Ok(home) => {
            setup_authorized_keys_in(Path::new(&home), ssh_fingerprint, script_path, pubkey)
        }
        Err(e) => Err(anyhow!("HOME is not usable: {e}")),
    }
}

/// [`setup_authorized_keys`] for the user whose home is `home`.
pub fn setup_authorized_keys_in(
    home: &Path,
    ssh_fingerprint: &str,
    script_path: &str,
    pubkey: &str,
) -> Result<()> {
    let authorized_keys = authorized_keys_path(home);
    let ssh_dir = match authorized_keys.parent() {
        Some(dir) => dir.to_path_buf(),
        None => {
            return Err(anyhow!(
                "no parent directory for {}",
                authorized_keys.display()
            ))
        }
    };

    if !ssh_dir.exists() {
        if let Err(e) = fs::create_dir_all(&ssh_dir) {
            return Err(anyhow!("failed to create {}: {e}", ssh_dir.display()));
        }
    }

    let mut file = match OpenOptions::new()
        .create(true)
        .append(true)
        .open(&authorized_keys)
    {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("failed to open {}: {e}", authorized_keys.display())),
    };

    if let Err(e) = writeln!(
        file,
        "command=\"FINGERPRINT={} NAME=default {}\",no-agent-forwarding,no-user-rc,no-X11-forwarding,no-port-forwarding {}",
        ssh_fingerprint, script_path, pubkey
    ) {
        return Err(anyhow!("failed to write {}: {e}", authorized_keys.display()));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(&ssh_dir, fs::Permissions::from_mode(0o700)) {
            return Err(anyhow!("failed to chmod {}: {e}", ssh_dir.display()));
        }
        if let Err(e) = fs::set_permissions(&authorized_keys, fs::Permissions::from_mode(0o600)) {
            return Err(anyhow!(
                "failed to chmod {}: {e}",
                authorized_keys.display()
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "ssh_keys_tests.rs"]
mod tests;
