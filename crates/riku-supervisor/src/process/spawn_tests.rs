use super::super::test_support::minimal_config;
use crate::process::ProcessManager;
use tempfile::TempDir;

#[test]
fn test_open_log_files_creates_file_and_dirs() {
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("subdir").join("app.log");
    let handles = ProcessManager::open_log_files(log_path.to_str().unwrap())
        .expect("open_log_files should succeed");
    assert!(handles.is_some(), "should return file handles");
    assert!(log_path.exists(), "log file should be created on disk");
}

#[test]
fn test_spawn_process_echo_succeeds() {
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("test.log");

    let config = minimal_config(
        "echo hello",
        tmp.path().to_str().unwrap(),
        log_path.to_str().unwrap(),
    );

    let mut pm = ProcessManager::new().expect("ProcessManager::new should succeed");
    pm.spawn_process(&config)
        .expect("spawning 'echo hello' should succeed");

    // Allow log-capture threads to drain before asserting count.
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(
        pm.get_process_count(),
        1,
        "one process should be registered"
    );
}

#[test]
fn test_spawn_duplicate_process_id_replaces_old() {
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("test.log");

    let config = minimal_config(
        "sleep 60",
        tmp.path().to_str().unwrap(),
        log_path.to_str().unwrap(),
    );

    let mut pm = ProcessManager::new().expect("ProcessManager::new should succeed");
    pm.spawn_process(&config)
        .expect("first spawn should succeed");
    assert_eq!(pm.get_process_count(), 1);

    // Spawning again with the same app/kind/ordinal replaces the old entry.
    pm.spawn_process(&config)
        .expect("second spawn should succeed");
    assert_eq!(
        pm.get_process_count(),
        1,
        "duplicate should replace, not add"
    );
}
