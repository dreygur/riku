use anyhow::{anyhow, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::git::repo::post_receive_hook;
use crate::util::display;

fn create_dir(dir: &Path) -> Result<()> {
    match fs::create_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to create {}: {e}", dir.display())),
    }
}

fn init_bare_repo(repo_dir: &Path) -> Result<()> {
    let output = match Command::new("git")
        .args(["init", "--bare"])
        .current_dir(repo_dir)
        .output()
    {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("failed to run git init: {e}")),
    };
    if !output.status.success() {
        return Err(anyhow!(
            "git init failed in {} ({}): {}",
            repo_dir.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn write_hook(hook: &Path) -> Result<()> {
    if let Err(e) = fs::write(hook, post_receive_hook()) {
        return Err(anyhow!("failed to write {}: {e}", hook.display()));
    }
    match fs::set_permissions(hook, fs::Permissions::from_mode(0o755)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to chmod {}: {e}", hook.display())),
    }
}

/// Create a new application (directory and git repository).
#[allow(clippy::question_mark)]
pub fn cmd_apps_create(paths: &RikuPaths, name: &str) -> Result<()> {
    let app = match crate::util::validate_app_name(name) {
        Ok(app) => app,
        Err(e) => return Err(anyhow!("invalid app name: {e}")),
    };

    if paths.app_root.join(&app).exists() {
        anyhow::bail!("app '{}' already exists", app);
    }

    let app_dir = paths.app_root.join(&app);
    if let Err(e) = create_dir(&app_dir) {
        return Err(e);
    }
    display::step_done(&format!("Created app directory: {}", app_dir.display()), "");

    let repo_dir = paths.git_root.join(format!("{}.git", app));
    if let Err(e) = create_dir(&repo_dir) {
        return Err(e);
    }
    if let Err(e) = init_bare_repo(&repo_dir) {
        return Err(e);
    }
    display::step_done(
        &format!("Created git repository: {}", repo_dir.display()),
        "",
    );

    let hooks_dir = repo_dir.join("hooks");
    if let Err(e) = create_dir(&hooks_dir) {
        return Err(e);
    }
    let post_receive = hooks_dir.join("post-receive");
    if let Err(e) = write_hook(&post_receive) {
        return Err(e);
    }
    display::step_done(&format!("Created git hook: {}", post_receive.display()), "");
    display::detail("");

    display::step_done(&format!("App '{}' created successfully!", app), "");
    display::detail("");
    super::list::print_first_deploy_hint();
    display::detail_err(&format!(
        "  The repo directory name is the app name, so name it '{app}'."
    ));
    display::detail("");

    Ok(())
}
