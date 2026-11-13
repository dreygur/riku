//! Supervisor lifecycle control for deployed applications.
//!
//! Handles starting, checking, and notifying the riku supervisor process,
//! as well as spawning app processes after deployment.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::config::RikuPaths;
use crate::supervisor_pid;
use crate::util::{display, ROOT_BINARY};

/// Whether some process holds the lock on `supervisor.pid`. A running
/// supervisor keeps an exclusive `flock` on it for its whole life (see
/// riku-supervisor `create_pid_file_with_lock`), so an unlocked file is left
/// over from a dead one, whatever pid it names: that pid may since belong to
/// an unrelated process. A supervisor starting in the probe's instant fails
/// its own lock.
pub(crate) fn pid_file_locked(paths: &RikuPaths) -> bool {
    supervisor_pid::probe(&paths.riku_root.join(supervisor_pid::PID_FILE))
        .is_some_and(|file| file.locked)
}

/// The pid of the running supervisor: the pid file must be locked and hold
/// a positive pid (see [`supervisor_pid`] for how it is read).
pub fn running_supervisor_pid(paths: &RikuPaths) -> Option<i32> {
    supervisor_pid::running_pid(&paths.riku_root.join(supervisor_pid::PID_FILE))
}

/// Return true when a supervisor holds its pid file lock.
pub(crate) fn supervisor_is_running(paths: &RikuPaths) -> bool {
    running_supervisor_pid(paths).is_some()
}

/// `installed` if it exists, else `current` (this process's executable),
/// else `None`. Never a bare name for the loader to look up on `$PATH`.
fn resolve_from(installed: &Path, current: Option<std::path::PathBuf>) -> Option<String> {
    if installed.exists() {
        return Some(installed.to_string_lossy().into_owned());
    }
    current.and_then(|p| p.to_str().map(str::to_string))
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
/// *actually currently running*, immune to `$PATH` content entirely. When
/// neither is known the result is `None` and the supervisor is not started.
pub(crate) fn resolve_riku_bin() -> Option<String> {
    resolve_from(Path::new(ROOT_BINARY), std::env::current_exe().ok())
}

/// Start the supervisor unless one holds its pid file lock, and return
/// whether one runs afterwards.
///
/// A stale pid file is left in place: the new supervisor rewrites it under
/// its lock, and removing it here could unlink the file a supervisor that is
/// starting right now has already opened.
pub(crate) fn ensure_supervisor_running(paths: &RikuPaths) -> bool {
    if supervisor_is_running(paths) {
        return true;
    }

    let Some(riku_bin) = resolve_riku_bin() else {
        tracing::error!(
            "cannot start the supervisor: no {} and the running binary's path is unknown",
            ROOT_BINARY
        );
        return false;
    };

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
            if supervisor_is_running(paths) {
                return true;
            }
        }
    }

    false
}

/// Notify the supervisor to reload configurations by sending SIGHUP to the
/// PID recorded in the PID file, only while a supervisor holds that file's
/// lock, so a pid reused by another process is never signalled.
///
/// The same signal makes the supervisor list containers at once, so
/// container deploys, scales and restarts send it after starting copies.
pub(crate) fn notify_supervisor_reload(paths: &RikuPaths) {
    if let Some(pid) = running_supervisor_pid(paths) {
        let _ = nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid),
            nix::sys::signal::Signal::SIGHUP,
        );
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
        display::warn("Could not start the supervisor. Run 'riku supervisor' manually.");
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
    Ok(())
}

#[cfg(test)]
#[path = "supervisor_ctl_tests.rs"]
mod tests;
