use super::*;
use crate::config::create_worker_config;
use std::collections::HashMap;
use tempfile::TempDir;

fn write_sleep_worker_config(config_dir: &std::path::Path, log_dir: &std::path::Path) {
    let config = create_worker_config(
        "sighuptest",
        "web",
        "sleep 60",
        1,
        HashMap::new(),
        "/tmp",
        log_dir.join("web.1.log").to_str().unwrap(),
    );
    let toml_str = toml::to_string(&config).unwrap();
    std::fs::write(config_dir.join("sighuptest-web-1.toml"), toml_str).unwrap();
}

/// End-to-end regression test for the SIGHUP hot-reload path: fires a
/// *real* `SIGHUP` at this test process via `nix::sys::signal::kill`
/// (not a direct function call), proving the async
/// `tokio::signal::unix` listener spawned by `spawn_sighup_listener`
/// actually catches process-level signal delivery: not just that the
/// reload logic works when called directly.
///
/// Also proves the reload is non-destructive: a worker whose config
/// file didn't change keeps the exact same PID across the reload, i.e.
/// `reload_all_configs`'s mtime diff against `watched_configs` (the
/// live process tree) correctly skips it rather than restarting
/// everything on every SIGHUP.
#[test]
fn test_sighup_triggers_reload_without_disturbing_unchanged_worker() {
    let tmp = TempDir::new().unwrap();
    let riku_root = tmp.path().join(".riku");
    let config_dir = riku_root.join("workers-enabled");
    let log_dir = riku_root.join("logs");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::create_dir_all(&log_dir).unwrap();

    write_sleep_worker_config(&config_dir, &log_dir);

    let mut supervisor = Supervisor::new(config_dir.clone()).unwrap();
    supervisor.load_initial_configs().unwrap();
    assert_eq!(
        supervisor.process_manager.get_process_count(),
        1,
        "the sleep worker should be spawned by load_initial_configs"
    );

    let pid_before = supervisor
        .process_manager
        .list_processes()
        .into_iter()
        .find(|p| p.process_id == "sighuptest-web-1")
        .expect("worker should be registered before reload")
        .pid;

    // Start the real async listener under test, then fire an actual
    // SIGHUP at this process: exercising real kernel signal delivery
    // end to end, not a synthetic counter bump.
    crate::spawn_sighup_listener();
    RELOAD_COUNTER.store(0, Ordering::SeqCst);

    nix::sys::signal::kill(nix::unistd::Pid::this(), nix::sys::signal::Signal::SIGHUP)
        .expect("failed to send SIGHUP to self");

    // The listener runs on its own thread/runtime asynchronously, so
    // poll briefly rather than assuming instant delivery.
    let mut caught = false;
    for _ in 0..50 {
        if RELOAD_COUNTER.load(Ordering::SeqCst) > 0 {
            caught = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        caught,
        "tokio::signal::unix SIGHUP listener did not observe the signal within 1s"
    );

    // Mirror exactly what the main loop does on a pending reload.
    RELOAD_COUNTER.store(0, Ordering::SeqCst);
    supervisor.reload_all_configs().unwrap();

    assert_eq!(
        supervisor.process_manager.get_process_count(),
        1,
        "reload must not have added or removed the worker"
    );
    let pid_after = supervisor
        .process_manager
        .list_processes()
        .into_iter()
        .find(|p| p.process_id == "sighuptest-web-1")
        .expect("worker should still be registered after reload")
        .pid;
    assert_eq!(
        pid_before, pid_after,
        "an unchanged worker config must not be restarted by a SIGHUP reload \
             (same PID before and after)"
    );

    supervisor.process_manager.stop_all_processes().unwrap();
}
