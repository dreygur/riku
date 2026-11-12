use anyhow::{bail, Result};
use std::fs;
use std::path::Path;

use crate::config::app_settings::{AppSettings, Source};
use crate::config::RikuPaths;
use crate::util::{copy_dir_recursive, count_files, display, exit_if_invalid};

/// Deploy an app.
// Explicit match/if-let over `?` per project convention (.claude/CLAUDE.md
// rule 16): every early exit stays visible as a `return Err(...)`.
#[allow(clippy::question_mark)]
pub fn cmd_deploy(paths: &RikuPaths, app: &str, from_path: Option<&str>) -> Result<()> {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };

    let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

    // If deploying from local path, copy files first (creates app directory)
    if let Some(source_path) = from_path {
        if let Err(e) = deploy_from_path(paths, &app, source_path) {
            return Err(e);
        }
    }

    crate::deploy::do_deploy(&app, paths, &deltas, None)
}

/// Whether `source` has a Procfile or a valid riku.toml with `[processes]`
/// or `[cron]`. An invalid riku.toml is an error.
#[allow(clippy::question_mark)]
fn declares_processes(source: &Path) -> Result<bool> {
    if source.join("Procfile").exists() {
        return Ok(true);
    }
    let file = source.join(crate::config::app_settings::FILE_NAME);
    match AppSettings::load(&file, Source::Repo) {
        Ok(Some(settings)) => Ok(settings.processes().is_some()),
        Ok(None) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Deploy from a local path (copies files to app directory).
#[allow(clippy::question_mark)]
fn deploy_from_path(paths: &RikuPaths, app: &str, source: &str) -> Result<()> {
    let source_path = Path::new(source);

    if !source_path.exists() {
        display::error(&format!("path '{}' does not exist.", source));
        bail!("Source path does not exist");
    }

    if !source_path.is_dir() {
        display::error(&format!("'{}' is not a directory.", source));
        bail!("Source is not a directory");
    }

    let declared = match declares_processes(source_path) {
        Ok(declared) => declared,
        Err(e) => return Err(e),
    };
    if !declared {
        display::error("no Procfile or riku.toml [processes] in source directory.");
        display::warn("Declare the processes to run in either one.");
        display::warn("Example: echo 'web: npm start' > Procfile");
        bail!("no processes declared");
    }

    let git_dir = source_path.join(".git");
    if !git_dir.exists() {
        display::warn("source is not a git repository.");
        display::warn("  Consider initializing git: git init");
    }

    let app_dir = paths.app_root.join(app);
    display::step_done(&format!("Copying files from '{}'...", source), "");

    // Copy to a temp dir first, then atomic rename
    let tmp_dir = paths.app_root.join(format!(".{}.tmp", app));
    if tmp_dir.exists() {
        if let Err(e) = fs::remove_dir_all(&tmp_dir) {
            return Err(anyhow::anyhow!("cannot remove {}: {e}", tmp_dir.display()));
        }
    }

    if let Err(e) = copy_dir_recursive(source_path, &tmp_dir) {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(e);
    }

    if app_dir.exists() {
        if let Err(e) = fs::remove_dir_all(&app_dir) {
            return Err(anyhow::anyhow!("cannot remove {}: {e}", app_dir.display()));
        }
    }

    if let Err(e) = fs::rename(&tmp_dir, &app_dir) {
        return Err(anyhow::anyhow!(
            "cannot move {} to {}: {e}",
            tmp_dir.display(),
            app_dir.display()
        ));
    }

    let copied = match count_files(&app_dir) {
        Ok(count) => count,
        Err(e) => return Err(e),
    };
    display::step_done(&format!("Copied {copied} files"), "");

    Ok(())
}
