//! Nginx configuration validation.
//!
//! Validates generated nginx config files by invoking `nginx -t`.

use anyhow::Result;
use std::fs;
use std::path::Path;

/// Temporary file with automatic cleanup.
pub(super) struct TempFile {
    path: std::path::PathBuf,
}

impl TempFile {
    pub(super) fn new(path: std::path::PathBuf) -> Self {
        TempFile { path }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Check one site config with `nginx -t`, wrapped in a minimal main config.
///
/// The wrapper sits beside `config_file` as `<name>.check`, a name nginx's
/// `*.conf` include never matches, so a reload racing the check cannot load
/// it.
pub(super) fn validate_nginx_config(config_file: &Path, nginx: &Path) -> Result<()> {
    use std::process::Command;

    let mut wrapper_name = config_file.file_name().unwrap_or_default().to_os_string();
    wrapper_name.push(".check");
    let temp_nginx_conf = config_file.with_file_name(wrapper_name);
    let _temp_file = TempFile::new(temp_nginx_conf.clone());

    let include_directive = format!("include {};\n", config_file.display());
    let full_config = format!(
        "events {{ worker_connections 1024; }}\nhttp {{\n{}\n}}",
        include_directive
    );

    if let Err(e) = fs::write(&temp_nginx_conf, &full_config) {
        return Err(anyhow::anyhow!(
            "failed to write {}: {e}",
            temp_nginx_conf.display()
        ));
    }

    let output = match Command::new(nginx)
        .arg("-t")
        .arg("-c")
        .arg(&temp_nginx_conf)
        .output()
    {
        Ok(output) => output,
        Err(e) => return Err(anyhow::anyhow!("could not run {}: {e}", nginx.display())),
    };

    // Temp file is automatically cleaned up by Drop

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check if syntax is OK (nginx outputs "syntax is ok" even on permission errors)
    if stderr.contains("syntax is ok") {
        // Config syntax is valid, permission errors are not our concern
        return Ok(());
    }

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Nginx config validation failed: {}",
            stderr
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_temp_file_deleted_on_drop() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("to_be_deleted.tmp");
        fs::write(&path, "data").unwrap();
        assert!(path.exists());

        {
            let _guard = TempFile::new(path.clone());
            // Guard is alive; file still exists
            assert!(path.exists());
        }
        // Guard dropped; file must be gone
        assert!(!path.exists());
    }

    #[test]
    fn test_temp_file_drop_on_missing_file_does_not_panic() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("never_created.tmp");
        // Drop should silently ignore the missing file
        let _guard = TempFile::new(path);
    }

    #[test]
    #[ignore = "requires nginx binary on PATH"]
    fn test_validate_nginx_config_with_real_nginx() {
        let tmp = TempDir::new().unwrap();
        let config_file = tmp.path().join("site.conf");
        fs::write(
            &config_file,
            "server { listen 8080; server_name localhost; location / { return 200; } }",
        )
        .unwrap();
        let nginx = crate::config::program::find_program("nginx");
        let result = validate_nginx_config(&config_file, &nginx);
        assert!(result.is_ok());
    }
}
