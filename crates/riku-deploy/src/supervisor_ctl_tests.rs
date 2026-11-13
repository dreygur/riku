use super::*;
use std::fs;
use tempfile::TempDir;

fn make_paths(tmp: &TempDir) -> RikuPaths {
    crate::config::RikuPaths::for_tests(tmp.path())
}

// --- resolve_riku_bin ---

/// The binary to relaunch is the root install, else the running binary's
/// absolute path; never a bare `riku` for `$PATH` to search. `resolve_from`
/// takes no environment, so no test here changes `PATH` or `HOME`: both are
/// process wide, and changing them made parallel tests fail to spawn `sleep`.
#[test]
fn relaunch_is_the_root_install_or_the_running_binary() {
    let tmp = TempDir::new().unwrap();
    let running = std::env::current_exe().unwrap();
    let not_installed = tmp.path().join("no-such-riku");
    let installed = tmp.path().join("riku");
    fs::write(&installed, "#!/bin/sh\n").unwrap();

    assert_eq!(
        resolve_from(&not_installed, Some(running.clone())),
        running.to_str().map(str::to_string),
        "with no root install the running binary is relaunched"
    );
    assert_eq!(
        resolve_from(&installed, Some(running)),
        installed.to_str().map(str::to_string),
        "the root install wins over the running binary"
    );
}

/// When the running binary's path is unknown and there is no root install,
/// there is no binary to start, rather than a bare `riku` for `$PATH`.
#[test]
fn unknown_current_exe_gives_no_binary_not_a_path_lookup() {
    let tmp = TempDir::new().unwrap();

    assert_eq!(resolve_from(&tmp.path().join("no-such-riku"), None), None);
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
