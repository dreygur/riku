use tempfile::TempDir;

use crate::daemon::Supervisor;

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
