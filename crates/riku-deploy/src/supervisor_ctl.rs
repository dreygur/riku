//! Supervisor lifecycle control for deployed applications.
//!
//! Handles starting, checking, and notifying the riku supervisor process,
//! as well as spawning app processes after deployment.

use anyhow::Result;
use std::collections::HashMap;

use crate::config::RikuPaths;
use crate::util::echo;

/// Read the supervisor PID from the PID file. Returns None if the file is absent,
/// unreadable, or contains a non-numeric value.
fn read_supervisor_pid(paths: &RikuPaths) -> Option<i32> {
    let pid_file = paths.riku_root.join("supervisor.pid");
    let content = std::fs::read_to_string(&pid_file).ok()?;
    let pid: i32 = content.trim().parse().ok()?;
    Some(pid)
}

/// Return true if a process with the given PID exists (sending signal 0).
fn pid_is_alive(pid: i32) -> bool {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_ok()
}

/// Resolve which `riku` binary to exec when (re-)launching the supervisor.
///
/// Order: `$RIKU_BIN` (explicit override) → `~/.local/bin/riku` (the
/// installed location, if present) → this process's own executable via
/// `std::env::current_exe()`.
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
    std::env::var("RIKU_BIN")
        .ok()
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            let local_bin = format!("{}/.local/bin/riku", home);
            if std::path::Path::new(&local_bin).exists() {
                local_bin
            } else {
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.to_str().map(str::to_string))
                    .unwrap_or_else(|| "riku".to_string())
            }
        })
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

    // Exec nohup directly: never interpolate riku_bin into a shell string to
    // avoid injection if RIKU_BIN contains metacharacters.
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

/// Notify the supervisor to reload configurations and spawn processes.
/// This function is called after deployment to start/restart application processes.
/// The worker configs should already exist from the deploy step.
pub fn spawn_app(app: &str, paths: &RikuPaths) -> Result<()> {
    let app_path = paths.app_root.join(app);

    // Get environment variables for nginx config generation
    let env_file = paths.env_root.join(app).join("ENV");
    let mut env: HashMap<String, String> = HashMap::new();
    if env_file.exists() {
        crate::util::parse_settings(&env_file, &mut env)?;
    }

    // Configure the active router (built-in nginx, or a router plugin)
    if let Err(e) = crate::router::configure(app, &app_path, &env, paths) {
        echo(
            &format!("Warning: Failed to configure router: {}", e),
            "yellow",
        );
    }

    // Ensure supervisor is running, start if not
    if !ensure_supervisor_running(paths) {
        echo(
            "Warning: Could not start supervisor. Run 'riku supervisor' manually.",
            "yellow",
        );
    }

    // Notify the supervisor to reload configurations
    // The supervisor will detect new/changed configs and spawn processes
    notify_supervisor_reload(paths);

    echo("-----> Notified supervisor to spawn processes...", "green");

    Ok(())
}

#[cfg(test)]
#[path = "supervisor_ctl_tests.rs"]
mod tests;
