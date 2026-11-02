//! Confirming a compose project name the app did not get from its own name.
//!
//! # Security model
//!
//! The project name prefixes every volume name, so it decides whose data an
//! app mounts. A name the compose file or app env declares is refused when it
//! equals another app's normalised name, or when an existing volume of that
//! name lacks the compose label of that project. Every failure to read the
//! app list or a volume refuses the name instead of skipping the check.

use std::fs;
use std::io::ErrorKind;

use anyhow::{anyhow, Result};
use riku_container::compose::{normalise_project_name, ComposeProject, ProjectNameSource};
use riku_container::engine::Engine;
use riku_container::types::labels;

use super::AppName;
use crate::config::RikuPaths;

fn check_other_apps(project: &str, app: &AppName, paths: &RikuPaths) -> Result<()> {
    let entries = match fs::read_dir(&paths.app_root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", paths.app_root.display())),
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => return Err(anyhow!("cannot read {}: {e}", paths.app_root.display())),
        };
        let other = entry.file_name().to_string_lossy().into_owned();
        if other != app.as_str()
            && entry.path().is_dir()
            && normalise_project_name(&other) == project
        {
            return Err(anyhow!(
                "app '{app}' declares project name '{project}', which is also app '{other}''s; set another name"
            ));
        }
    }
    Ok(())
}

fn check_volume_labels(project: &ComposeProject, engine: &Engine) -> Result<()> {
    let name = project.project_name();
    for key in project.volumes().keys() {
        let volume = format!("{name}_{key}");
        match engine.volume_exists(&volume) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(e) => return Err(e),
        }
        let volume_labels = match engine.volume_labels(&volume) {
            Ok(volume_labels) => volume_labels,
            Err(e) => return Err(e),
        };
        if volume_labels
            .get(labels::COMPOSE_PROJECT)
            .map(String::as_str)
            != Some(name)
        {
            return Err(anyhow!(
                "volume '{volume}' exists but was not created by compose for project '{name}'; refusing to mount it"
            ));
        }
    }
    Ok(())
}

/// Accept a `Declared` project name only when no other app normalises to it
/// (`Shop`, `sh.op` and `shop` all compare as `shop`) and every existing
/// volume `<project>_<key>` carries `com.docker.compose.project=<project>`.
/// Names that are not `Declared` pass unchanged.
pub fn confirm_project_name(
    project: &mut ComposeProject,
    app: &AppName,
    paths: &RikuPaths,
    engine: &Engine,
) -> Result<()> {
    if project.name_source() != ProjectNameSource::Declared {
        return Ok(());
    }
    if let Err(e) = check_other_apps(project.project_name(), app, paths) {
        return Err(e);
    }
    if let Err(e) = check_volume_labels(project, engine) {
        return Err(e);
    }
    project.confirm_declared_name();
    Ok(())
}
