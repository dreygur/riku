use crate::config::{WorkerConfig, WorkerInfo, WorkerOptions};
use crate::process::ProcessManager;
use std::collections::HashMap;
use tempfile::TempDir;

fn sleep_config(tmp: &TempDir) -> WorkerConfig {
    let log = tmp.path().join("out.log");
    WorkerConfig {
        worker: WorkerInfo {
            app: "testapp".to_string(),
            kind: "web".to_string(),
            command: "sleep 60".to_string(),
            ordinal: 1,
        },
        env: HashMap::new(),
        options: WorkerOptions {
            working_dir: tmp.path().to_str().unwrap().to_string(),
            log_file: log.to_str().unwrap().to_string(),
            uid: None,
            gid: None,
            timeout: 30,
            // short grace period so the test doesn't wait 30 s
            grace_period: 1,
            max_restarts: 3,
            health_check: None,
            limits: None,
            sandbox: None,
        },
    }
}

#[test]
fn test_stop_app_processes_removes_them_from_manager() {
    let tmp = TempDir::new().unwrap();
    let config = sleep_config(&tmp);

    let mut pm = ProcessManager::new().unwrap();
    pm.spawn_process(&config).expect("spawn should succeed");
    assert_eq!(pm.get_process_count(), 1);

    pm.stop_app_processes("testapp")
        .expect("stop_app_processes should not fail");

    assert_eq!(
        pm.get_process_count(),
        0,
        "all processes for 'testapp' should be removed after stop"
    );
}

#[test]
fn test_stop_all_processes_clears_manager() {
    let tmp = TempDir::new().unwrap();
    let mut config = sleep_config(&tmp);

    let mut pm = ProcessManager::new().unwrap();

    // Spawn two different workers.
    pm.spawn_process(&config).expect("spawn 1");
    config.worker.ordinal = 2;
    pm.spawn_process(&config).expect("spawn 2");
    assert_eq!(pm.get_process_count(), 2);

    pm.stop_all_processes()
        .expect("stop_all_processes should not fail");
    assert_eq!(pm.get_process_count(), 0, "all processes should be removed");
}

#[test]
fn test_stop_nonexistent_process_is_noop() {
    let tmp = TempDir::new().unwrap();
    let _ = tmp; // keep alive

    let mut pm = ProcessManager::new().unwrap();
    // Stopping a process that was never registered must not panic or error.
    pm.stop_app_processes("nonexistent-app")
        .expect("stopping a nonexistent app should be a silent no-op");
}

#[test]
fn stop_kills_group_members_that_outlive_the_leader_before_dropping_the_marker() {
    let tmp = TempDir::new().unwrap();
    let pid_dir = tmp.path().join("pids");
    let mut config = sleep_config(&tmp);
    config.worker.command = "(trap '' TERM; sleep 60) & sleep 60".to_string();

    let mut pm = ProcessManager::new().unwrap().with_pid_dir(pid_dir.clone());
    pm.spawn_process(&config).expect("spawn");
    let process_id = config.worker.name().to_string();
    let pgid = pm.processes[&process_id].pid;

    pm.stop_process_by_id(&process_id).expect("stop");

    let group = nix::sys::signal::kill(nix::unistd::Pid::from_raw(-pgid.as_raw()), None);
    assert_eq!(
        group,
        Err(nix::errno::Errno::ESRCH),
        "the TERM-ignoring group member must be gone once stop returns"
    );
    let markers: Vec<_> = std::fs::read_dir(&pid_dir).unwrap().collect();
    assert!(markers.is_empty(), "marker left behind: {markers:?}");
}

#[test]
fn dropping_a_crashed_worker_stops_the_group_members_it_left() {
    let tmp = TempDir::new().unwrap();
    let mut config = sleep_config(&tmp);
    config.worker.command = "(trap '' TERM; sleep 60) & exit 1".to_string();

    let mut pm = ProcessManager::new().unwrap();
    pm.spawn_process(&config).expect("spawn");
    let mut crashed = pm
        .processes
        .remove(&config.worker.name().to_string())
        .unwrap();
    let pgid = crashed.pid;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while crashed.is_running() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    drop(crashed);

    let group = nix::sys::signal::kill(nix::unistd::Pid::from_raw(-pgid.as_raw()), None);
    assert_eq!(
        group,
        Err(nix::errno::Errno::ESRCH),
        "the member left by the crashed leader must be gone once it is dropped"
    );
}
