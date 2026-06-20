//! SSH authorized_keys management utilities.

use anyhow::Result;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

/// Append to ~/.ssh/authorized_keys with SSH restrictions.
/// Set directory permissions to 700, file permissions to 600.
///
/// `scope` and `apps` are optional per-key restrictions consumed by the CLI
/// authorization gate (`cli::authz`) via the `RIKU_AGENT_SCOPE` /
/// `RIKU_KEY_APPS` env vars injected into the forced command. When both are
/// `None`, the key gets the same unrestricted line written before per-key
/// scoping existed — legacy keys are unaffected.
pub fn setup_authorized_keys(
    ssh_fingerprint: &str,
    script_path: &str,
    pubkey: &str,
    scope: Option<&str>,
    apps: Option<&[String]>,
) -> Result<()> {
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

    let mut restrictions = format!("FINGERPRINT={} NAME=default", ssh_fingerprint);
    if let Some(scope) = scope {
        restrictions.push_str(&format!(" RIKU_AGENT_SCOPE={}", scope));
    }
    if let Some(apps) = apps {
        restrictions.push_str(&format!(" RIKU_KEY_APPS={}", apps.join(",")));
    }

    writeln!(
        file,
        "command=\"{} {} $SSH_ORIGINAL_COMMAND\",no-agent-forwarding,no-user-rc,no-X11-forwarding,no-port-forwarding {}",
        restrictions, script_path, pubkey
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
            let result = setup_authorized_keys(
                "fingerprint123",
                "/usr/bin/riku",
                "ssh-rsa AAAA user@host",
                None,
                None,
            );
            assert!(result.is_ok());
            assert!(tmp.path().join(".ssh").exists());
            assert!(tmp.path().join(".ssh/authorized_keys").exists());
        });
    }

    #[test]
    fn test_setup_appends_correct_line_format() {
        with_home(|tmp| {
            setup_authorized_keys(
                "fp42",
                "/usr/local/bin/riku",
                "ssh-rsa BBBB test@local",
                None,
                None,
            )
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
            setup_authorized_keys("fp1", "/usr/bin/riku", "ssh-rsa KEY1 a@b", None, None).unwrap();
            setup_authorized_keys("fp2", "/usr/bin/riku", "ssh-rsa KEY2 c@d", None, None).unwrap();
            let content = fs::read_to_string(tmp.path().join(".ssh/authorized_keys")).unwrap();
            assert!(content.contains("KEY1"));
            assert!(content.contains("KEY2"));
            assert_eq!(content.lines().count(), 2);
        });
    }

    #[test]
    fn test_setup_writes_scope_and_apps_when_provided() {
        with_home(|tmp| {
            let apps = vec!["demoapp".to_string(), "otherapp".to_string()];
            setup_authorized_keys(
                "fp-scoped",
                "/usr/bin/riku",
                "ssh-rsa SCOPED test@host",
                Some("readonly"),
                Some(&apps),
            )
            .unwrap();
            let content = fs::read_to_string(tmp.path().join(".ssh/authorized_keys")).unwrap();
            assert!(content.contains("RIKU_AGENT_SCOPE=readonly"));
            assert!(content.contains("RIKU_KEY_APPS=demoapp,otherapp"));
        });
    }

    #[test]
    fn test_setup_omits_scope_and_apps_when_absent() {
        with_home(|tmp| {
            setup_authorized_keys(
                "fp-legacy",
                "/usr/bin/riku",
                "ssh-rsa LEGACY a@b",
                None,
                None,
            )
            .unwrap();
            let content = fs::read_to_string(tmp.path().join(".ssh/authorized_keys")).unwrap();
            assert!(!content.contains("RIKU_AGENT_SCOPE"));
            assert!(!content.contains("RIKU_KEY_APPS"));
        });
    }

    #[test]
    #[cfg(unix)]
    fn test_setup_sets_correct_permissions() {
        use std::os::unix::fs::PermissionsExt;
        with_home(|tmp| {
            setup_authorized_keys("fp", "/usr/bin/riku", "ssh-rsa KEY user@host", None, None)
                .unwrap();
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
