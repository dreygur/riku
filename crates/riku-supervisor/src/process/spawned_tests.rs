use crate::process::test_support::minimal_config;
use crate::process::ProcessManager;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::thread;
use std::time::Duration;

/// A supervisor running as PID 1 inherits the group members its workers
/// leave behind. A member spawned by the test itself is the same situation:
/// an exited member that is our child and stays a zombie until reaped.
#[test]
#[allow(
    clippy::zombie_processes,
    reason = "group_alive is what reaps the member"
)]
fn group_alive_counts_exited_members_that_are_our_children_as_gone() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("out.log");
    let config = minimal_config(
        "sleep 0.5",
        tmp.path().to_str().unwrap(),
        log.to_str().unwrap(),
    );
    let mut pm = ProcessManager::new().unwrap();
    pm.spawn_process(&config).expect("spawn");
    let mut process = pm
        .processes
        .remove(&config.worker.name().to_string())
        .unwrap();

    let _member = Command::new("true")
        .process_group(process.pid.as_raw())
        .spawn()
        .expect("join the worker's group");
    thread::sleep(Duration::from_millis(200));
    process.child.wait().expect("reap the leader");

    assert!(
        !process.group_alive(),
        "an exited member that is our child kept the group alive"
    );
}
