use anyhow::Result;
use std::fs;
use std::io::{self, BufRead};
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::{echo, sanitize_app_name};

/// Post-receive git hook handler.
pub fn cmd_git_hook(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = sanitize_app_name(app);
    let repo_path = paths.git_root.join(&app);
    let app_path = paths.app_root.join(&app);
    let data_path = paths.data_root.join(&app);

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let _oldrev = parts[0];
        let newrev = parts[1];
        let _refname = parts[2];

        if !app_path.exists() {
            echo(&format!("-----> Creating app '{}'", app), "green");
            fs::create_dir_all(&app_path)?;
            if !data_path.exists() {
                fs::create_dir_all(&data_path)?;
            }
            let status = Command::new("git")
                .arg("clone")
                .arg("--quiet")
                .arg(&repo_path)
                .arg(&app)
                .current_dir(&paths.app_root)
                .status()?;
            if !status.success() {
                echo("Error: git clone failed.", "red");
            }
        }

        // Call the actual deploy function
        let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        crate::deploy::do_deploy(&app, paths, &deltas, Some(newrev))?;
    }

    Ok(())
}

/// Handle git pushes for an app. Sets up bare repo and hook if needed,
/// then delegates to git-shell.
pub fn cmd_git_receive_pack(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = sanitize_app_name(app);
    let hook_path = paths.git_root.join(&app).join("hooks").join("post-receive");

    if !hook_path.exists() {
        // Create hooks directory
        if let Some(parent) = hook_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Initialize bare repo
        let status = Command::new("git")
            .arg("init")
            .arg("--quiet")
            .arg("--bare")
            .arg(&app)
            .current_dir(&paths.git_root)
            .status()?;
        if !status.success() {
            echo("Error: git init failed.", "red");
        }

        // Write post-receive hook
        let hook_content = format!(
            "#!/usr/bin/env bash\nset -e; set -o pipefail;\ncat | PIKU_ROOT=\"{piku_root}\" {piku_script} git-hook {app}",
            piku_root = paths.riku_root.display(),
            piku_script = paths.riku_script.display(),
            app = app,
        );
        fs::write(&hook_path, hook_content)?;

        // Make hook executable
        let meta = fs::metadata(&hook_path)?;
        let mode = meta.permissions().mode();
        fs::set_permissions(&hook_path, fs::Permissions::from_mode(mode | 0o755))?;
    }

    // Delegate to git-shell for the actual receive
    let shell_cmd = format!("git-receive-pack '{}'", app);
    let status = Command::new("git-shell")
        .arg("-c")
        .arg(&shell_cmd)
        .current_dir(&paths.git_root)
        .status()?;

    std::process::exit(status.code().unwrap_or(1));
}

/// Handle git upload-pack for an app.
pub fn cmd_git_upload_pack(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = sanitize_app_name(app);

    let shell_cmd = format!("git-upload-pack '{}'", app);
    let status = Command::new("git-shell")
        .arg("-c")
        .arg(&shell_cmd)
        .current_dir(&paths.git_root)
        .status()?;

    std::process::exit(status.code().unwrap_or(1));
}
