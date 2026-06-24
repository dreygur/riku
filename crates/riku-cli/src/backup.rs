//! `riku backup` / `riku restore` provider layer.

use std::path::Path;

use anyhow::Result;

use crate::config::RikuPaths;
use crate::deploy::backup::BackupService;
use crate::util::display;

/// `riku backup <app> [--out <path>]`
pub fn cmd_backup(paths: &RikuPaths, app: &str, out: Option<&str>) -> Result<()> {
    display::info(&format!("Backing up '{app}'..."));
    let path = BackupService::new(paths).backup(app, out.map(Path::new))?;
    display::success(&format!("Backed up '{app}' to {}", path.display()));
    Ok(())
}

/// `riku restore <app> <file>`
pub fn cmd_restore(paths: &RikuPaths, app: &str, file: &str) -> Result<()> {
    display::info(&format!("Restoring '{app}' from {file}..."));
    BackupService::new(paths).restore(app, Path::new(file))?;
    display::success(&format!("Restored '{app}'."));
    display::note(&format!(
        "Bring it up with: riku deploy {app}  (or: riku restart {app})"
    ));
    Ok(())
}
