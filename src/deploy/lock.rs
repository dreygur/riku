//! Per-app deploy lock.
//!
//! `do_deploy` mutates state shared across concurrent triggers for the same
//! app: it `git reset --hard`s the working tree, rewrites worker TOML
//! configs, and read-modify-writes the ENV file's nginx port allocation.
//! Two deploys for the same app racing (a second `git push` landing while
//! the first `post-receive` hook is still running, or a dashboard-triggered
//! redeploy firing mid-push) corrupt that shared state — e.g. a lost update
//! on `NGINX_INTERNAL_PORT` leaves nginx proxying to a port no worker is
//! bound to. An advisory `flock` keyed by app name serializes deploys of the
//! same app without affecting deploys of different apps.

use anyhow::Result;
use std::fs::{self, File, OpenOptions};
use std::os::unix::io::AsRawFd;

use crate::config::RikuPaths;
use crate::error::DeployError;

/// Acquire the deploy lock for `app`, non-blocking. Returns the locked file
/// handle — the lock is held until it is dropped, so callers must keep the
/// returned `File` alive for the duration of the deploy.
///
/// Returns `Err(DeployError::DeployInProgress)` if another deploy for this
/// app already holds the lock.
pub(super) fn acquire(app: &str, paths: &RikuPaths) -> Result<File> {
    let lock_dir = paths.riku_root.join("locks");
    fs::create_dir_all(&lock_dir)?;
    let lock_path = lock_dir.join(format!("{}.deploy.lock", app));

    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)?;

    // Use libc::flock directly, same as the supervisor's PID-file lock
    // (create_pid_file_with_lock) — portable across Unix systems, no extra
    // dependency.
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result != 0 {
        return Err(DeployError::DeployInProgress(app.to_string()).into());
    }

    Ok(file)
}

#[allow(dead_code)]
fn lock_path_for(app: &str, paths: &RikuPaths) -> std::path::PathBuf {
    paths.riku_root.join("locks").join(format!("{}.deploy.lock", app))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn make_paths(tmp: &TempDir) -> RikuPaths {
        RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path())
    }

    #[test]
    fn test_acquire_succeeds_when_unlocked() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        assert!(acquire("myapp", &paths).is_ok());
    }

    #[test]
    fn test_acquire_fails_when_already_locked() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);

        let _first = acquire("myapp", &paths).expect("first lock should succeed");
        let second = acquire("myapp", &paths);

        assert!(second.is_err(), "second concurrent lock should fail");
        let err = second.unwrap_err();
        assert!(
            matches!(
                err.downcast_ref::<DeployError>(),
                Some(DeployError::DeployInProgress(_))
            ),
            "error should be DeployError::DeployInProgress, got: {:?}",
            err
        );
    }

    #[test]
    fn test_acquire_succeeds_again_after_lock_dropped() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);

        {
            let _first = acquire("myapp", &paths).expect("first lock should succeed");
        } // dropped here, releasing the flock

        assert!(
            acquire("myapp", &paths).is_ok(),
            "lock should be acquirable again once released"
        );
    }

    #[test]
    fn test_acquire_different_apps_do_not_conflict() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);

        let _a = acquire("app-a", &paths).expect("app-a lock should succeed");
        let _b = acquire("app-b", &paths).expect("app-b lock should succeed independently");
    }

    #[test]
    fn test_lock_path_includes_app_name() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let path = lock_path_for("myapp", &paths);
        assert!(path.to_string_lossy().contains("myapp"));
    }

    #[test]
    fn test_lock_file_persists_on_disk() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let _lock = acquire("myapp", &paths).unwrap();
        assert!(Path::new(&lock_path_for("myapp", &paths)).exists());
    }
}
