//! SSH authorized_keys management utilities.

use anyhow::{anyhow, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::authorized_keys::authorized_keys_path;

/// Append to ~/.ssh/authorized_keys with SSH restrictions.
/// Set directory permissions to 700, file permissions to 600.
pub fn setup_authorized_keys(ssh_fingerprint: &str, script_path: &str, pubkey: &str) -> Result<()> {
    let home = match std::env::var("HOME") {
        Ok(home) => home,
        Err(e) => return Err(anyhow!("HOME is not usable: {e}")),
    };
    let authorized_keys = authorized_keys_path(Path::new(&home));
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
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Serialize all tests that mutate the process-global HOME env var.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_home<F: FnOnce(&TempDir)>(f: F) {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let original_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", tmp.path());
        f(&tmp);
        match original_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn test_setup_creates_ssh_dir_and_authorized_keys() {
        with_home(|tmp| {
            let result =
                setup_authorized_keys("fingerprint123", "/usr/bin/riku", "ssh-rsa AAAA user@host");
            assert!(result.is_ok());
            assert!(tmp.path().join(".ssh").exists());
            assert!(tmp.path().join(".ssh/authorized_keys").exists());
        });
    }

    #[test]
    fn test_setup_appends_correct_line_format() {
        with_home(|tmp| {
            setup_authorized_keys("fp42", "/usr/local/bin/riku", "ssh-rsa BBBB test@local")
                .unwrap();
            let content = fs::read_to_string(tmp.path().join(".ssh/authorized_keys")).unwrap();
            assert!(content.contains("FINGERPRINT=fp42"));
            assert!(content.contains("/usr/local/bin/riku"));
            assert!(content.contains("ssh-rsa BBBB test@local"));
            assert!(content.contains("no-agent-forwarding"));
            assert!(content.contains("no-port-forwarding"));
        });
    }

    #[test]
    fn test_setup_appends_multiple_keys() {
        with_home(|tmp| {
            setup_authorized_keys("fp1", "/usr/bin/riku", "ssh-rsa KEY1 a@b").unwrap();
            setup_authorized_keys("fp2", "/usr/bin/riku", "ssh-rsa KEY2 c@d").unwrap();
            let content = fs::read_to_string(tmp.path().join(".ssh/authorized_keys")).unwrap();
            assert!(content.contains("KEY1"));
            assert!(content.contains("KEY2"));
            assert_eq!(content.lines().count(), 2);
        });
    }

    #[test]
    fn new_keys_get_plain_forced_command() {
        with_home(|tmp| {
            setup_authorized_keys("fp", "/usr/bin/riku", "ssh-rsa KEY user@host").unwrap();
            let content = fs::read_to_string(tmp.path().join(".ssh/authorized_keys")).unwrap();
            assert!(
                !content.contains("$SSH_ORIGINAL_COMMAND"),
                "forced command must not expand the shell variable: {content}"
            );
            assert!(content.contains(r#"command="FINGERPRINT=fp NAME=default /usr/bin/riku","#));
        });
    }

    #[test]
    #[cfg(unix)]
    fn test_setup_sets_correct_permissions() {
        use std::os::unix::fs::PermissionsExt;
        with_home(|tmp| {
            setup_authorized_keys("fp", "/usr/bin/riku", "ssh-rsa KEY user@host").unwrap();
            let ssh_dir = tmp.path().join(".ssh");
            let dir_mode = fs::metadata(&ssh_dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(dir_mode, 0o700, "ssh dir should be 700");
            let file_mode = fs::metadata(ssh_dir.join("authorized_keys"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(file_mode, 0o600, "authorized_keys should be 600");
        });
    }
}
