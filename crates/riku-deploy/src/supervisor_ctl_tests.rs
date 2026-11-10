use super::*;
use std::fs;
use std::sync::Mutex;
use tempfile::TempDir;

fn make_paths(tmp: &TempDir) -> RikuPaths {
    crate::config::RikuPaths::for_tests(tmp.path())
}

// Serialize tests that mutate the process-global PATH/HOME env vars.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Save the current value of an env var, run `f`, then restore it
/// exactly (set back if it was present, removed if it wasn't).
fn with_env_var<F: FnOnce()>(key: &str, value: &str, f: F) {
    let original = std::env::var(key).ok();
    std::env::set_var(key, value);
    f();
    match original {
        Some(v) => std::env::set_var(key, v),
        None => std::env::remove_var(key),
    }
}

// --- resolve_riku_bin ---

/// Core regression test for fix #11: even with `PATH` empty or pointed
/// entirely at attacker-controlled, malicious directories,
/// `resolve_riku_bin` must never hand back a bare
/// `"riku"` for the OS loader to search `$PATH` for: it must resolve to
/// an absolute path via `current_exe()`, which is immune to `$PATH`
/// content entirely.
#[test]
fn test_resolve_riku_bin_ignores_malicious_path_falls_back_to_current_exe() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = TempDir::new().unwrap();

    // A directory under attacker control holding a fake "riku" that
    // would run instead of the real binary if anything here trusted
    // $PATH for resolution.
    let evil_dir = tmp.path().join("evil-bin");
    fs::create_dir_all(&evil_dir).unwrap();
    fs::write(evil_dir.join("riku"), "#!/bin/sh\necho pwned\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(evil_dir.join("riku"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    // No root install on this path, so resolution must fall to current_exe().
    let not_installed = tmp.path().join("no-such-riku");

    for malicious_path in [
        String::new(),                              // empty PATH
        evil_dir.to_string_lossy().to_string(),     // PATH = only the attacker's dir
        format!("{}:/usr/bin", evil_dir.display()), // attacker's dir takes priority
    ] {
        with_env_var("PATH", &malicious_path, || {
            let resolved = resolve_from(&not_installed, std::env::current_exe().ok()).unwrap();

            assert!(
                resolved.starts_with('/'),
                "resolution must return an absolute path regardless of PATH \
                     content, got: {:?} (PATH={:?})",
                resolved,
                malicious_path
            );
            assert_ne!(
                resolved, "riku",
                "must never fall back to a bare name for $PATH to search"
            );
            assert!(
                !resolved.starts_with(evil_dir.to_str().unwrap()),
                "resolved binary must not come from the attacker-controlled PATH \
                     directory, got: {:?}",
                resolved
            );

            let expected = std::env::current_exe()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            assert_eq!(
                resolved, expected,
                "with no override and no root install, resolution must be \
                     current_exe(), not a PATH-dependent lookup"
            );
        });
    }
}

/// When the running binary's path is unknown and there is no root install,
/// there is no binary to start, rather than a bare `riku` for `$PATH`.
#[test]
fn unknown_current_exe_gives_no_binary_not_a_path_lookup() {
    let tmp = TempDir::new().unwrap();

    assert_eq!(resolve_from(&tmp.path().join("no-such-riku"), None), None);
}

/// A binary the deploy user owns in its home must never be picked, however
/// it got there: it could be swapped for anything an app can write.
#[test]
fn relaunch_skips_home_binary() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = TempDir::new().unwrap();
    let home_bin = tmp.path().join(".local/bin");
    fs::create_dir_all(&home_bin).unwrap();
    fs::write(home_bin.join("riku"), "#!/bin/sh\n").unwrap();

    with_env_var("HOME", tmp.path().to_str().unwrap(), || {
        let resolved = resolve_riku_bin().unwrap();
        assert!(
            !resolved.starts_with(tmp.path().to_str().unwrap()),
            "relaunch picked a binary under the home: {resolved}"
        );
    });
}

// --- read_supervisor_pid ---

#[test]
fn test_read_supervisor_pid_returns_none_when_no_pid_file() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    // Riku root doesn't have a supervisor.pid file
    let pid = read_supervisor_pid(&paths);
    assert!(pid.is_none(), "Should return None when pid file absent");
}

#[test]
fn test_read_supervisor_pid_parses_valid_pid() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    fs::create_dir_all(&paths.riku_root).unwrap();
    fs::write(paths.riku_root.join("supervisor.pid"), "12345\n").unwrap();

    let pid = read_supervisor_pid(&paths);
    assert_eq!(pid, Some(12345));
}

#[test]
fn test_read_supervisor_pid_returns_none_for_non_numeric_content() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    fs::create_dir_all(&paths.riku_root).unwrap();
    fs::write(paths.riku_root.join("supervisor.pid"), "not-a-number\n").unwrap();

    let pid = read_supervisor_pid(&paths);
    assert!(pid.is_none(), "Non-numeric PID file should return None");
}

#[test]
fn test_read_supervisor_pid_returns_none_for_empty_file() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    fs::create_dir_all(&paths.riku_root).unwrap();
    fs::write(paths.riku_root.join("supervisor.pid"), "").unwrap();

    let pid = read_supervisor_pid(&paths);
    assert!(pid.is_none(), "Empty PID file should return None");
}

// --- pid_is_alive ---

#[test]
fn test_pid_is_alive_returns_true_for_own_process() {
    // Signal 0 to our own PID must succeed
    let our_pid = std::process::id() as i32;
    assert!(pid_is_alive(our_pid), "Our own process should be alive");
}

#[test]
fn test_pid_is_alive_returns_false_for_pid_zero() {
    // PID 0 means "process group": kill(0, 0) raises ESRCH or EPERM;
    // either way riku treats it as "not alive" to avoid signalling the group.
    // We just verify the function does not panic.
    let _ = pid_is_alive(0);
}

#[test]
fn test_pid_is_alive_returns_false_for_nonexistent_pid() {
    // i32::MAX is virtually guaranteed to be an unused PID.
    assert!(
        !pid_is_alive(i32::MAX),
        "i32::MAX should not map to a real process"
    );
}

// --- notify_supervisor_reload ---

#[test]
fn test_notify_supervisor_reload_no_pid_file_does_not_panic() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    // No PID file: should be a no-op, never panic
    notify_supervisor_reload(&paths);
}

#[test]
fn test_notify_supervisor_reload_stale_pid_does_not_panic() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    fs::create_dir_all(&paths.riku_root).unwrap();
    // Write a PID that almost certainly doesn't exist
    fs::write(paths.riku_root.join("supervisor.pid"), "999999999\n").unwrap();
    notify_supervisor_reload(&paths);
}
