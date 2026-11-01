use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::process::{Command, Stdio};

use super::control_target::{control_target, ControlTarget};
use crate::config::{RikuPaths, RIKU_RAW_SOURCE_URL};
use crate::supervisor::Supervisor;
use crate::util::{display, exit_if_invalid, parse_settings};

/// Run a command in the app context with LIVE_ENV loaded.
pub fn cmd_run(paths: &RikuPaths, app: &str, cmd: &[String]) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("LIVE_ENV");
    let mut env = HashMap::new();
    parse_settings(&config_file, &mut env)?;

    let app_dir = paths.app_root.join(&app);

    if cmd.is_empty() {
        anyhow::bail!("no command specified for 'riku run'");
    }

    // Exec the binary directly rather than rejoining into `sh -c` to avoid
    // shell injection via metacharacters in the command arguments.
    let mut child = Command::new(&cmd[0])
        .args(&cmd[1..])
        .current_dir(&app_dir)
        .envs(&env)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    child.wait()?;
    Ok(())
}

/// Restart an app. A container app replaces the copies of `service`, or of
/// every service, one at a time from the images it runs now; a Procfile app
/// is stopped and deployed again. Naming a service of a Procfile app fails.
#[allow(clippy::question_mark)]
pub fn cmd_restart(paths: &RikuPaths, app: &str, service: Option<&str>) -> Result<()> {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    let target = match control_target(paths, &app, service) {
        Ok(target) => target,
        Err(e) => return Err(e),
    };
    display::info(&format!("restarting app '{}'...", app));
    match target {
        ControlTarget::Containers { app, service } => {
            crate::deploy::containers::restart_service(&app, paths, service.as_ref())
        }
        ControlTarget::Procfile => {
            do_stop(paths, &app);
            crate::deploy::do_deploy(&app, paths, &HashMap::new(), None)
        }
    }
}

/// Stop an app. A container app stops and removes the copies of `service`,
/// or of every service, keeping its volumes; a Procfile app loses its
/// enabled worker configs. Naming a service of a Procfile app fails.
#[allow(clippy::question_mark)]
pub fn cmd_stop(paths: &RikuPaths, app: &str, service: Option<&str>) -> Result<()> {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    match control_target(paths, &app, service) {
        Ok(ControlTarget::Containers { app, service }) => {
            crate::deploy::containers::stop_service(&app, paths, service.as_ref())
        }
        Ok(ControlTarget::Procfile) => {
            do_stop(paths, &app);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Self-update the binary by downloading latest from RIKU_RAW_SOURCE_URL.
pub fn cmd_update() -> Result<()> {
    display::info("Updating riku...");

    let output = Command::new("curl")
        .args([
            "-sL",
            "-w",
            "%{http_code}",
            RIKU_RAW_SOURCE_URL,
            "-o",
            "/dev/null",
        ])
        .output()?;

    let http_code = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if http_code == "200" {
        display::warn("Note: self-update for riku binary is not yet implemented.");
        display::note("The reference source is accessible.");
    } else {
        display::note(&format!(
            "Error updating riku - please check if {} is accessible from this machine.",
            RIKU_RAW_SOURCE_URL
        ));
    }
    display::success("Done.");
    Ok(())
}

/// Start the supervisor daemon.
/// For production use, prefer 'riku supervisor --daemon' or the systemd service.
pub fn cmd_supervisor(paths: &RikuPaths) -> Result<()> {
    let mut supervisor = Supervisor::new(paths.workers_enabled.clone())?.with_actions(
        std::sync::Arc::new(crate::control_actions::CliControlActions),
    );
    supervisor.run()
}

/// Stop an app by removing its enabled worker config files.
fn do_stop(paths: &RikuPaths, app: &str) {
    let mut configs: Vec<std::path::PathBuf> = Vec::new();

    for ext in &["ini", "toml"] {
        let pattern = paths.workers_enabled.join(format!("{}-*.{}", app, ext));
        if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
            for entry in entries.flatten() {
                configs.push(entry);
            }
        }
    }

    if !configs.is_empty() {
        display::info(&format!("Stopping app '{}'...", app));
        for c in &configs {
            if let Err(e) = fs::remove_file(c) {
                tracing::warn!("Could not remove worker config {:?}: {}", c, e);
            }
        }
    } else {
        display::error(&format!("Error: app '{}' not deployed!", app));
    }
}
