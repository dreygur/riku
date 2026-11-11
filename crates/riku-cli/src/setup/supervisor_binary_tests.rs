use std::fs;
use std::path::Path;

use super::*;

/// `<proc>/<pid>/exe` pointing at `binary`, as the kernel shows it.
fn running_from(proc_root: &Path, pid: u32, binary: &Path) {
    let dir = proc_root.join(pid.to_string());
    fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(binary, dir.join("exe")).unwrap();
}

#[test]
fn rerunning_init_without_an_upgrade_keeps_the_supervisor_running() {
    let tmp = tempfile::tempdir().unwrap();
    let installed = tmp.path().join("riku");
    fs::write(&installed, "riku 0.1.4").unwrap();
    let proc_root = tmp.path().join("proc");
    running_from(&proc_root, 41, &installed);
    // A byte for byte copy, as left by an install from another path of the
    // same release.
    let copy = tmp.path().join("riku-copy");
    fs::write(&copy, "riku 0.1.4").unwrap();
    running_from(&proc_root, 42, &copy);

    assert_eq!(
        action_for(Some(41), &proc_root, &installed),
        ServiceAction::KeepRunning
    );
    assert_eq!(
        action_for(Some(42), &proc_root, &installed),
        ServiceAction::KeepRunning,
        "the same bytes under another inode are not a new binary"
    );
    assert_eq!(ServiceAction::KeepRunning.verb(), "start");
    assert_eq!(ServiceAction::KeepRunning.outcome(), "running");
}

#[test]
fn a_new_binary_restarts_the_supervisor() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old-riku");
    fs::write(&old, "riku 0.1.3").unwrap();
    let installed = tmp.path().join("riku");
    fs::write(&installed, "riku 0.1.4").unwrap();
    let proc_root = tmp.path().join("proc");
    running_from(&proc_root, 41, &old);

    let action = action_for(Some(41), &proc_root, &installed);

    assert_eq!(action, ServiceAction::Restart);
    assert_eq!(action.verb(), "restart");
    assert_eq!(action.outcome(), "restarted (new binary)");
}

#[test]
fn a_stopped_service_is_started() {
    let tmp = tempfile::tempdir().unwrap();

    assert_eq!(parse_main_pid("0\n"), None, "systemd prints 0 when stopped");
    assert_eq!(parse_main_pid("1234\n"), Some(1234));
    assert_eq!(
        action_for(None, tmp.path(), &tmp.path().join("riku")),
        ServiceAction::Start
    );
}
