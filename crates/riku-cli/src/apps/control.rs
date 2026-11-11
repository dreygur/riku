use anyhow::Result;
use std::collections::HashMap;
use std::process::{Command, Stdio};

use super::control_target::{control_target, ControlTarget};

use crate::config::RikuPaths;
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

    let status = match child.wait() {
        Ok(status) => status,
        Err(e) => return Err(anyhow::anyhow!("waiting for '{}' failed: {e}", cmd[0])),
    };
    if !status.success() {
        anyhow::bail!("'{}' exited with {status}", cmd[0]);
    }
    Ok(())
}

/// Restart an app without building it. A container app replaces the copies
/// of `service`, or of every service, one at a time from the images it runs
/// now; a Procfile app's supervisor restarts its workers from the current
/// build. Naming a service of a Procfile app fails. A rebuild is
/// `riku deploy`.
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
        ControlTarget::Procfile => restart_workers(paths, &app),
    }
}

fn restart_workers(paths: &RikuPaths, app: &str) -> Result<()> {
    match crate::deploy::app_restart::restart_procfile_app(app, paths) {
        Ok(count) => {
            display::step_done("Restarted", &format!("{count} worker(s) of '{app}'"));
            Ok(())
        }
        Err(e) => Err(e),
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
        Ok(ControlTarget::Procfile) => do_stop(paths, &app),
        Err(e) => Err(e),
    }
}

/// Start the supervisor daemon.
/// For production use, prefer 'riku supervisor --daemon' or the systemd service.
pub fn cmd_supervisor(paths: &RikuPaths) -> Result<()> {
    let mut supervisor = Supervisor::new(paths.workers_enabled.clone())?.with_actions(
        std::sync::Arc::new(crate::control_actions::CliControlActions),
    );
    supervisor.run()
}

/// Stop an app by removing its worker configs and its copies' sockets.
fn do_stop(paths: &RikuPaths, app: &str) -> Result<()> {
    match crate::deploy::app_stop::stop_procfile_app(app, paths) {
        Ok(0) => display::error(&format!("app '{}' is not deployed", app)),
        Ok(_) => display::info(&format!("Stopped app '{}'.", app)),
        Err(e) => return Err(anyhow::anyhow!("cannot stop '{app}': {e}")),
    }
    Ok(())
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
