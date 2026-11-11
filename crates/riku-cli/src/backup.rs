//! `riku backup` / `riku restore` provider layer.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::path::Path;

use anyhow::Result;

use crate::config::RikuPaths;
use crate::deploy::backup::BackupService;
use crate::util::display;

/// `riku backup <app> [--out <path>]`
pub fn cmd_backup(paths: &RikuPaths, app: &str, out: Option<&str>) -> Result<()> {
    display::intro(&format!("riku backup {app}"));
    let path = match BackupService::new(paths).backup(app, out.map(Path::new)) {
        Ok(path) => path,
        Err(e) => return Err(e),
    };
    let archive = path.display().to_string();
    display::step_done("Archive", &archive);
    display::outro("Backed up");
    display::summary(&[
        ("File", &archive),
        ("Restore", &format!("riku restore {app} {archive}")),
    ]);
    Ok(())
}

/// `riku restore <app> <file>`
pub fn cmd_restore(paths: &RikuPaths, app: &str, file: &str) -> Result<()> {
    display::intro(&format!("riku restore {app}"));
    if let Err(e) = BackupService::new(paths).restore(app, Path::new(file)) {
        return Err(e);
    }
    display::step_done("Restored", file);
    display::outro("Restored, not running yet");
    display::summary(&[(
        "Start",
        &format!("riku deploy {app}  (or: riku restart {app})"),
    )]);
    Ok(())
}
