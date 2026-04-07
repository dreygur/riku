//! Git post-receive hook handler — triggers deployment on push.

use anyhow::Result;
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::echo;

/// Post-receive git hook handler.
pub fn cmd_git_hook(paths: &RikuPaths, app: &str, repo_path: Option<&str>) -> Result<()> {
    let app = crate::util::validate_app_name(app)?;

    // If repo_path is provided, create symlink from ~/.riku/repos/{app}.git to actual location
    if let Some(actual_repo) = repo_path {
        let actual_path = Path::new(actual_repo);
        if actual_path.exists() {
            let riku_repo = paths.git_root.join(format!("{}.git", app));
            if !riku_repo.exists() {
                std::os::unix::fs::symlink(actual_path, &riku_repo)?;
                echo(
                    &format!(
                        "Symlinked {} → {}",
                        riku_repo.display(),
                        actual_path.display()
                    ),
                    "green",
                );
            }
        }
    }

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

        // Clone repo if app directory doesn't exist or is empty (no Procfile)
        if !app_path.exists() || !app_path.join("Procfile").exists() {
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
