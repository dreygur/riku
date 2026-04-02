//! SSH authorized_keys management utilities.

use anyhow::Result;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

/// Append to ~/.ssh/authorized_keys with SSH restrictions.
/// Set directory permissions to 700, file permissions to 600.
pub fn setup_authorized_keys(ssh_fingerprint: &str, script_path: &str, pubkey: &str) -> Result<()> {
    let home = std::env::var("HOME")?;
    let ssh_dir = Path::new(&home).join(".ssh");
    let authorized_keys = ssh_dir.join("authorized_keys");

    if !ssh_dir.exists() {
        fs::create_dir_all(&ssh_dir)?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&authorized_keys)?;

    writeln!(
        file,
        "command=\"FINGERPRINT={} NAME=default {} $SSH_ORIGINAL_COMMAND\",no-agent-forwarding,no-user-rc,no-X11-forwarding,no-port-forwarding {}",
        ssh_fingerprint, script_path, pubkey
    )?;

    // Set permissions: dir 700, file 600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&ssh_dir, fs::Permissions::from_mode(0o700))?;
        fs::set_permissions(&authorized_keys, fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}
