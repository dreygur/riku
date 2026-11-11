use tempfile::TempDir;

use crate::daemon::Supervisor;

/// `riku restart` probes the pid file with an instant shared lock. A
/// supervisor starting in that instant must wait it out, not exit.
#[test]
fn start_waits_out_a_brief_liveness_probe() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".riku").join("workers-enabled");
    std::fs::create_dir_all(&config_dir).unwrap();
    let supervisor = Supervisor::new(config_dir).unwrap();
    std::fs::write(&supervisor.pid_file, "").unwrap();
    let probe = std::fs::File::open(&supervisor.pid_file).unwrap();
    probe.lock_shared().unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        drop(probe);
    });

    let started = supervisor.create_pid_file_with_lock(4242);

    release.join().unwrap();
    assert!(started.is_ok(), "start gave up during a probe: {started:?}");
}

/// A starter that opened the pid file just before a clean exit must compete
/// for the same lock as the next start. Unlinking the file on exit left it
/// locking an orphaned inode while the next start locked a fresh file: two
/// supervisors.
#[test]
fn a_clean_exit_keeps_the_pid_file_so_every_start_locks_one_inode() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".riku").join("workers-enabled");
    std::fs::create_dir_all(&config_dir).unwrap();
    let mut first = Supervisor::new(config_dir.clone()).unwrap();
    first.pid_file_lock = Some(first.create_pid_file_with_lock(4242).unwrap());
    let opened_before_exit = std::fs::File::open(&first.pid_file).unwrap();

    first.shutdown().unwrap();
    let next = Supervisor::new(config_dir).unwrap();
    let _held = next.create_pid_file_with_lock(4343).unwrap();

    assert!(
        opened_before_exit.try_lock().is_err(),
        "the early starter got a lock beside the running supervisor"
    );
}

#[test]
fn second_supervisor_leaves_the_running_ones_pid_in_place() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".riku").join("workers-enabled");
    std::fs::create_dir_all(&config_dir).unwrap();
    let first = Supervisor::new(config_dir.clone()).unwrap();
    let second = Supervisor::new(config_dir).unwrap();

    let _held = first.create_pid_file_with_lock(4242).unwrap();
    let refused = second.create_pid_file_with_lock(4343);

    assert!(refused.is_err(), "the pid file is locked by the first");
    assert_eq!(
        std::fs::read_to_string(&first.pid_file).unwrap(),
        "4242\n",
        "a refused start must not empty the running supervisor's pid file"
    );
}
