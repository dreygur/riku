//! Supervisor lifecycle control for deployed applications.
//!
//! Handles starting, checking, and notifying the riku supervisor process,
//! as well as spawning app processes after deployment.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::config::RikuPaths;
use crate::util::{echo, ROOT_BINARY};

/// Read the supervisor PID from the PID file. Returns None if the file is absent,
/// unreadable, or contains a non-numeric value.
fn read_supervisor_pid(paths: &RikuPaths) -> Option<i32> {
    let pid_file = paths.riku_root.join("supervisor.pid");
    let content = std::fs::read_to_string(&pid_file).ok()?;
    let pid: i32 = content.trim().parse().ok()?;
    Some(pid)
}

/// Return true if a process with the given PID exists (sending signal 0).
pub(crate) fn pid_is_alive(pid: i32) -> bool {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_ok()
}

/// Return true when the supervisor PID file holds a live process id.
pub(crate) fn supervisor_is_running(paths: &RikuPaths) -> bool {
    match read_supervisor_pid(paths) {
        Some(pid) => pid_is_alive(pid),
        None => false,
    }
}

fn resolve_from(installed: &Path) -> String {
    if installed.exists() {
        return installed.to_string_lossy().into_owned();
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(str::to_string))
        .unwrap_or_else(|| "riku".to_string())
}

/// Resolve which `riku` binary to exec when (re-)launching the supervisor.
///
/// Order: the root owned install at [`ROOT_BINARY`], if present → this
/// process's own executable via `std::env::current_exe()`. Neither the
/// environment nor the home directory is consulted.
///
/// Deliberately never falls back to a bare `"riku"` resolved by the loader
/// off `$PATH`. The supervisor is (re)started from inside a git hook's exec
/// environment: if an attacker can influence `$PATH` there (e.g. via a
/// crafted `ENV` that reaches this process, or a hook running with an
/// unsanitized shell environment), a bare-name lookup would let them get an
/// arbitrary binary executed as the deploy user. `current_exe()` resolves
/// via `/proc/self/exe` (Linux): an absolute path to the binary that is
/// *actually currently running*, immune to `$PATH` content entirely.
pub(crate) fn resolve_riku_bin() -> String {
    resolve_from(Path::new(ROOT_BINARY))
}

/// Check if supervisor is running (via PID file) and start it if not.
pub(crate) fn ensure_supervisor_running(paths: &RikuPaths) -> bool {
    // Check PID file first: reliable and does not match unrelated processes.
    if let Some(pid) = read_supervisor_pid(paths) {
        if pid_is_alive(pid) {
            return true;
        }
        // Stale PID file: supervisor died without cleaning up. Remove it.
        let _ = std::fs::remove_file(paths.riku_root.join("supervisor.pid"));
    }

    // Supervisor is not running, try to start it.
    let riku_bin = resolve_riku_bin();

    let riku_root = paths.riku_root.to_str().unwrap_or("/root/.riku");

    // Exec nohup directly: never interpolate riku_bin into a shell string.
    if std::process::Command::new("nohup")
        .args([&riku_bin, "supervisor"])
        .env("RIKU_ROOT", riku_root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok()
    {
        // Poll PID file for up to 3 seconds instead of a blind sleep.
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Some(pid) = read_supervisor_pid(paths) {
                if pid_is_alive(pid) {
                    return true;
                }
            }
        }
    }

    false
}

/// Notify the supervisor to reload configurations by sending SIGHUP to the
/// PID recorded in the PID file. Only signals our own supervisor process.
///
/// The same signal makes the supervisor list containers at once, so
/// container deploys, scales and restarts send it after starting copies.
pub(crate) fn notify_supervisor_reload(paths: &RikuPaths) {
    if let Some(pid) = read_supervisor_pid(paths) {
        if pid_is_alive(pid) {
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid),
                nix::sys::signal::Signal::SIGHUP,
            );
        }
    }
}

/// The app's merged env (repo riku.toml, ENV, server override), empty when
/// it has none.
pub(crate) fn read_app_env(app: &str, paths: &RikuPaths) -> Result<HashMap<String, String>> {
    match crate::config::app_config::load_env(paths, app) {
        Ok(loaded) => Ok(loaded.env),
        Err(e) => Err(anyhow::anyhow!(
            "failed to read the config of '{app}': {e:#}"
        )),
    }
}

/// Starts the supervisor unless it runs, and tells the user when it could
/// not be started.
pub(crate) fn start_supervisor(paths: &RikuPaths) {
    if !ensure_supervisor_running(paths) {
        echo(
            "Warning: Could not start supervisor. Run 'riku supervisor' manually.",
            "yellow",
        );
    }
}

/// Notify the supervisor to reload configurations and spawn processes.
/// This function is called after deployment to start/restart application processes.
/// The worker configs should already exist from the deploy step.
#[allow(clippy::question_mark)]
pub fn spawn_app(app: &str, paths: &RikuPaths) -> Result<()> {
    let app_path = paths.app_root.join(app);

    let env = match read_app_env(app, paths) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };

    // Configure the active router (built-in nginx, or a router plugin)
    if let Err(e) = crate::router::configure(app, &app_path, &env, paths, None) {
        return Err(anyhow::anyhow!("failed to configure router: {e}"));
    }

    start_supervisor(paths);

    // Notify the supervisor to reload configurations
    // The supervisor will detect new/changed configs and spawn processes
    notify_supervisor_reload(paths);

    echo("-----> Notified supervisor to spawn processes...", "green");

    Ok(())
}

#[cfg(test)]
#[path = "supervisor_ctl_tests.rs"]
mod tests;
