use anyhow::{bail, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::{display, exit_if_invalid};

/// Deploy an app.
pub fn cmd_deploy(paths: &RikuPaths, app: &str, from_path: Option<&str>) -> Result<()> {
    let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

    // If deploying from local path, copy files first (creates app directory)
    if let Some(source_path) = from_path {
        deploy_from_path(paths, app, source_path)?;
    } else if is_bare_repo() {
        deploy_from_bare_repo(paths, app)?;
    } else {
        let _ = exit_if_invalid(app, &paths.app_root)?;
    }

    crate::deploy::do_deploy(app, paths, &deltas, None)
}

/// Check if current directory is a bare git repo.
fn is_bare_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-bare-repository"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim() == "true")
        .unwrap_or(false)
}

/// Deploy from a bare repo by extracting files and setting up auto-deploy hook.
fn deploy_from_bare_repo(paths: &RikuPaths, app: &str) -> Result<()> {
    let bare_repo = std::env::current_dir()?;
    crate::cli::git::ensure_repo_symlink(paths, app)?;
    crate::cli::git::extract_bare_repo_to_app(&bare_repo, app, paths)?;
    crate::cli::git::setup_post_receive_hook(&bare_repo, app)?;
    Ok(())
}

/// Deploy from a local path (copies files to app directory).
fn deploy_from_path(paths: &RikuPaths, app: &str, source: &str) -> Result<()> {
    let source_path = Path::new(source);

    if !source_path.exists() {
        display::error(&format!("Error: path '{}' does not exist.", source));
        bail!("Source path does not exist");
    }

    if !source_path.is_dir() {
        display::error(&format!("Error: '{}' is not a directory.", source));
        bail!("Source is not a directory");
    }

    let procfile = source_path.join("Procfile");
    if !procfile.exists() {
        display::error("Error: Procfile not found in source directory.");
        display::warn("A Procfile is required for deployment.");
        display::warn("Example: echo 'web: npm start' > Procfile");
        bail!("Procfile not found");
    }

    let git_dir = source_path.join(".git");
    if !git_dir.exists() {
        display::warn("Warning: source is not a git repository.");
        display::warn("  Consider initializing git: git init");
    }

    let app_dir = paths.app_root.join(app);
    display::info(&format!("Copying files from '{}'...", source));

    // Copy to a temp dir first, then atomic rename
    let tmp_dir = paths.app_root.join(format!(".{}.tmp", app));
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
    }

    if let Err(e) = copy_dir_recursive(source_path, &tmp_dir) {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(e);
    }

    if app_dir.exists() {
        fs::remove_dir_all(&app_dir)?;
    }

    fs::rename(&tmp_dir, &app_dir)?;

    display::success(&format!("Copied {} files", count_files(&app_dir)?));

    Ok(())
}

/// Recursively copy a directory.
fn copy_dir_recursive(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let entry_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if entry_path.is_dir() {
            if entry_path
                .file_name()
                .map(|n| n == ".git" || n == "node_modules" || n == ".gitignore")
                .unwrap_or(false)
            {
                continue;
            }
            copy_dir_recursive(&entry_path, &dest_path)?;
        } else {
            fs::copy(&entry_path, &dest_path)?;
        }
    }

    Ok(())
}

/// Count files in a directory.
fn count_files(dir: &Path) -> Result<usize> {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_files(&path)?;
            } else {
                count += 1;
            }
        }
    }
    Ok(count)
}
