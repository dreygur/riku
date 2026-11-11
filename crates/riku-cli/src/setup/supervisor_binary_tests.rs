use std::fs;
use std::path::Path;

use super::*;

fn systemctl_output(code: i32, stdout: &str) -> std::io::Result<std::process::Output> {
    use std::os::unix::process::ExitStatusExt;
    Ok(std::process::Output {
        status: std::process::ExitStatus::from_raw(code << 8),
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    })
}

/// A query that failed (no user bus, no systemctl, garbage) is not a
/// stopped unit: reading it as one would SIGTERM the unit's own supervisor.
#[test]
fn a_failed_main_pid_query_is_an_error_not_a_stopped_unit() {
    let no_bus = systemctl_output(1, "");
    let not_found = Err(std::io::Error::from(std::io::ErrorKind::NotFound));
    let garbage = systemctl_output(0, "Failed to connect to bus\n");

    for (case, output) in [
        ("no bus", no_bus),
        ("no systemctl", not_found),
        ("garbage", garbage),
    ] {
        assert!(main_pid_from(output).is_err(), "{case} must be an error");
    }
}

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
        action_for(Some(41), UnitChange::Same, &proc_root, &installed),
        ServiceAction::KeepRunning
    );
    assert_eq!(
        action_for(Some(42), UnitChange::Same, &proc_root, &installed),
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

    let action = action_for(Some(41), UnitChange::Same, &proc_root, &installed);

    assert_eq!(action, ServiceAction::Restart(RestartReason::NewBinary));
    assert_eq!(action.verb(), "restart");
    assert_eq!(action.outcome(), "restarted (new binary)");
}

#[test]
fn a_rewritten_unit_restarts_a_supervisor_on_the_installed_binary() {
    let tmp = tempfile::tempdir().unwrap();
    let installed = tmp.path().join("riku");
    fs::write(&installed, "riku 0.1.4").unwrap();
    let proc_root = tmp.path().join("proc");
    running_from(&proc_root, 41, &installed);
    let unit = tmp.path().join("riku.service");

    assert_eq!(
        write_unit(&unit, "old").unwrap(),
        UnitChange::Changed,
        "new file"
    );
    assert_eq!(write_unit(&unit, "old").unwrap(), UnitChange::Same);
    let change = write_unit(&unit, "Delegate=yes").unwrap();

    let action = action_for(Some(41), change, &proc_root, &installed);
    assert_eq!(action, ServiceAction::Restart(RestartReason::NewUnit));
    assert_eq!(action.outcome(), "restarted (unit changed)");
    assert_eq!(fs::read_to_string(&unit).unwrap(), "Delegate=yes");
}

#[test]
fn a_stopped_service_is_started() {
    let tmp = tempfile::tempdir().unwrap();

    assert_eq!(
        main_pid_from(systemctl_output(0, "0\n")).unwrap(),
        None,
        "systemd prints 0 when stopped"
    );
    assert_eq!(
        main_pid_from(systemctl_output(0, "1234\n")).unwrap(),
        Some(1234)
    );
    assert_eq!(
        action_for(
            None,
            UnitChange::Changed,
            tmp.path(),
            &tmp.path().join("riku")
        ),
        ServiceAction::Start
    );
}
