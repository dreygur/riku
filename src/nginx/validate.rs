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

/// Validate nginx configuration file.
pub(super) fn validate_nginx_config(config_file: &Path) -> Result<()> {
    use std::process::Command;

    // Create a temporary nginx.conf that includes our site config
    // This allows proper validation of server block configs
    let temp_nginx_conf = config_file.with_file_name("test_nginx.conf");
    let _temp_file = TempFile::new(temp_nginx_conf.clone());

    let include_directive = format!("include {};\n", config_file.display());
    let full_config = format!(
        "events {{ worker_connections 1024; }}\nhttp {{\n{}\n}}",
        include_directive
    );

    fs::write(&temp_nginx_conf, &full_config)?;

    let output = Command::new("nginx")
        .arg("-t")
        .arg("-c")
        .arg(&temp_nginx_conf)
        .output()?;

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
