use std::os::unix::fs::symlink;

use super::*;
use crate::doctor::Status;

fn supervisor_running(exe: &str) -> (tempfile::TempDir, Vec<Check>) {
    let tmp = tempfile::tempdir().unwrap();
    let pid_file = tmp.path().join("supervisor.pid");
    fs::write(&pid_file, "4242\n").unwrap();
    let held = fs::File::open(&pid_file).unwrap();
    held.lock().unwrap();
    let proc_dir = tmp.path().join("proc/4242");
    fs::create_dir_all(&proc_dir).unwrap();
    symlink(exe, proc_dir.join("exe")).unwrap();
    let checks = supervisor_binary(&pid_file, &tmp.path().join("proc"));
    (tmp, checks)
}

/// A supervisor that exited leaves its pid file, unlocked; that pid may
/// belong to anything by now.
#[test]
fn an_unlocked_pid_file_reports_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let pid_file = tmp.path().join("supervisor.pid");
    fs::write(&pid_file, "4242\n").unwrap();

    assert!(supervisor_binary(&pid_file, &tmp.path().join("proc")).is_empty());
}

#[test]
fn installed_binary_passes() {
    let (_tmp, checks) = supervisor_running(ROOT_BINARY);
    assert_eq!(checks[0].status, Status::Ok, "{}", checks[0].detail);
}

#[test]
fn replaced_binary_fails_with_the_restart_command() {
    let (_tmp, checks) = supervisor_running("/usr/local/bin/riku (deleted)");
    assert_eq!(checks[0].status, Status::Fail, "{}", checks[0].detail);
    assert!(checks[0].detail.contains(FIX), "{}", checks[0].detail);
}

#[test]
fn other_path_fails() {
    let (_tmp, checks) = supervisor_running("/home/deploy/.local/bin/riku");
    assert_eq!(checks[0].status, Status::Fail, "{}", checks[0].detail);
}

#[test]
fn no_pid_file_reports_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(supervisor_binary(&tmp.path().join("supervisor.pid"), tmp.path()).is_empty());
}
