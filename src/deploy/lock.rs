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
pub(crate) fn acquire(app: &str, paths: &RikuPaths) -> Result<File> {
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

fn lock_path_for(app: &str, paths: &RikuPaths) -> std::path::PathBuf {
    paths
        .riku_root
        .join("locks")
        .join(format!("{}.deploy.lock", app))
}

/// Read-only probe: is `app`'s deploy lock currently held by another
/// process? Used by `riku __dump-state` to report live lock state without
/// ever taking the lock itself.
///
/// Implemented as a non-blocking `flock` attempt that's immediately
/// released on success — the only race-free way to ask "is this locked"
/// without disturbing a genuine holder: open a *fresh* fd (never the
/// holder's), try `LOCK_EX | LOCK_NB`. Success means nobody held it (and
/// this probe's own momentary lock is dropped immediately after); `EWOULDBLOCK`
/// means somebody does.
///
/// Returns `false` (not held) if the lock file doesn't exist yet, or if it
/// can't be opened at all — under-reporting "free" is the safe default for
/// a monitoring dump, since this is informational only and never gates a
/// real deploy decision.
pub(crate) fn is_locked(app: &str, paths: &RikuPaths) -> bool {
    let lock_path = lock_path_for(app, paths);
    let file = match OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
    {
        Ok(f) => f,
        Err(_) => return false,
    };

    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        // We got it — nobody else holds it. Release immediately; dropping
        // `file` closes the fd, which releases the flock too, but we
        // unlock explicitly first so there's no window where this probe
        // itself looks like a held lock to a concurrent probe.
        unsafe {
            libc::flock(file.as_raw_fd(), libc::LOCK_UN);
        }
        false
    } else {
        true
    }
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

    // --- is_locked ---

    #[test]
    fn test_is_locked_false_when_never_acquired() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        assert!(!is_locked("myapp", &paths));
    }

    #[test]
    fn test_is_locked_true_while_held() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let _held = acquire("myapp", &paths).unwrap();
        assert!(is_locked("myapp", &paths));
    }

    #[test]
    fn test_is_locked_false_after_release() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        {
            let _held = acquire("myapp", &paths).unwrap();
            assert!(is_locked("myapp", &paths));
        }
        assert!(!is_locked("myapp", &paths));
    }

    #[test]
    fn test_is_locked_probe_does_not_itself_hold_the_lock() {
        // Calling is_locked() on a free lock must not leave it held for a
        // subsequent real acquire() — the probe releases what it took.
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        assert!(!is_locked("myapp", &paths));
        assert!(
            acquire("myapp", &paths).is_ok(),
            "a probe must never leave the lock held behind it"
        );
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
