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
    let checks = supervisor_binary(Runner::DeployUser, &pid_file, &tmp.path().join("proc"));
    (tmp, checks)
}

/// A supervisor that exited leaves its pid file, unlocked; that pid may
/// belong to anything by now.
#[test]
fn an_unlocked_pid_file_reports_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let pid_file = tmp.path().join("supervisor.pid");
    fs::write(&pid_file, "4242\n").unwrap();

    assert!(supervisor_binary(Runner::DeployUser, &pid_file, &tmp.path().join("proc")).is_empty());
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
    assert!(supervisor_binary(
        Runner::DeployUser,
        &tmp.path().join("supervisor.pid"),
        tmp.path()
    )
    .is_empty());
}

/// Root never reads the deploy user's pid file, so a running supervisor
/// would read as none; the check must say it was skipped and how to run it.
#[test]
fn as_root_the_check_is_skipped_with_the_deploy_user_command() {
    let tmp = tempfile::tempdir().unwrap();
    let root = Runner::Root {
        deploy_user: "riku",
    };

    let checks = supervisor_binary(root, &tmp.path().join("supervisor.pid"), tmp.path());

    assert_eq!(checks.len(), 1, "one skipped line, not silence");
    assert_eq!(checks[0].status, Status::Warn, "{}", checks[0].detail);
    assert!(
        checks[0].detail.contains("Run as riku: riku doctor"),
        "{}",
        checks[0].detail
    );
}
