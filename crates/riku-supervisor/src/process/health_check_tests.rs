use super::super::test_support::minimal_config;
use super::ProcessManager;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::Mutex;
use tempfile::TempDir;

// `emit_app_restarted`/`emit_app_failed` read `RIKU_ROOT` via
// `RikuPaths::from_env()`, and the notifier plugin reads
// `RIKU_NOTIFY_WEBHOOK_URL`: both are process env vars. Serialize tests
// in this module that touch them so they don't race each other (no other
// test in this crate reads these names).
static ENV_GUARD: Mutex<()> = Mutex::new(());

/// Installs the real `plugins/riku-notify` bundle into `riku_root`,
/// exactly as `riku install-plugins --plugins riku-notify` would, not a
/// test fixture standing in for it.
fn install_real_notify_plugin(riku_root: &Path) {
    // CARGO_MANIFEST_DIR = .../riku/crates/riku-supervisor
    let repo_bundle = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/riku-notify");
    let dest = riku_root.join("plugins").join("riku-notify");
    std::fs::create_dir_all(dest.join("bin")).unwrap();
    std::fs::copy(
        repo_bundle.join("riku-plugin.toml"),
        dest.join("riku-plugin.toml"),
    )
    .unwrap();
    std::fs::copy(repo_bundle.join("bin/on-event"), dest.join("bin/on-event")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dest.join("bin/on-event"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
}

/// Spins up a one-shot HTTP listener on `127.0.0.1:0` and returns its
/// port plus a join handle yielding the raw request once received (or
/// `None` past its own deadline): so a plugin that never fires a
/// regression fails the test loudly instead of hanging the suite.
fn one_shot_webhook_listener() -> (u16, std::thread::JoinHandle<Option<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
                    return Some(String::from_utf8_lossy(&buf[..n]).to_string());
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() > deadline {
                        return None;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(_) => return None,
            }
        }
    });
    (port, handle)
}

/// A real crash, detected through the actual `check_processes()` entry
/// point (not a hand-rolled shortcut), must reach the real
/// `plugins/riku-notify` bundle and deliver a webhook with the crashed
/// process's real exit code, instance id, and restart count.
#[test]
fn crash_triggers_app_restarted_event_through_the_real_notify_plugin() {
    let _guard = ENV_GUARD.lock().unwrap();

    let tmp = TempDir::new().unwrap();
    let riku_root = tmp.path().join(".riku");
    install_real_notify_plugin(&riku_root);

    let (port, received) = one_shot_webhook_listener();
    std::env::set_var("RIKU_ROOT", &riku_root);
    std::env::set_var(
        "RIKU_NOTIFY_WEBHOOK_URL",
        format!("http://127.0.0.1:{port}/incident"),
    );

    let log_path = tmp.path().join("test.log");
    let config = minimal_config(
        "sh -c 'exit 42'",
        tmp.path().to_str().unwrap(),
        log_path.to_str().unwrap(),
    );

    let mut pm = ProcessManager::new().expect("ProcessManager::new should succeed");
    pm.spawn_process(&config).expect("spawn should succeed");

    // Poll the real production entry point until the crash is detected
    // and restarted, rather than reaching into internals to force it.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        pm.check_processes()
            .expect("check_processes should not error");
        let restarted = pm
            .processes
            .get("testapp-web-1")
            .map(|p| p.restart_count >= 1)
            .unwrap_or(false);
        if restarted {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "crash was never detected and restarted"
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    // Do NOT clear the env vars yet: `emit_app_restarted` (called from
    // inside `restart_process()`, above) spawns its own background
    // thread that reads them independently of this thread's timing,
    // clearing them here raced that thread and made this test hang
    // (it fell back to `$HOME/.riku` with no webhook URL configured,
    // so the plugin never fired and `received.join()` blocked forever).
    // Keep them live until the webhook has actually been observed.
    let request = received
        .join()
        .expect("webhook listener thread should not panic")
        .expect("plugin never delivered the webhook within the deadline");

    std::env::remove_var("RIKU_NOTIFY_WEBHOOK_URL");
    std::env::remove_var("RIKU_ROOT");

    assert!(request.contains("POST /incident"), "got: {request}");
    assert!(
        request.contains("\"event\":\"app.restarted\""),
        "got: {request}"
    );
    assert!(request.contains("\"app\":\"testapp\""), "got: {request}");
    assert!(
        request.contains("\"instance\":\"testapp-web-1\""),
        "got: {request}"
    );
    assert!(request.contains("\"exit_code\":42"), "got: {request}");
    assert!(request.contains("\"restart_count\":1"), "got: {request}");
}

/// A crash that exceeds `max_restarts` never reaches `restart_process()`,
/// it goes straight to removal, so `app.failed` has its own emit site
/// (`emit_app_failed`, called from the `__remove__` branch). Verify it
/// actually fires, with `max_restarts: 0` so the very first crash
/// detection is already "gave up" and there's no backoff to wait out.
#[test]
fn exhausted_retries_triggers_app_failed_event_through_the_real_notify_plugin() {
    let _guard = ENV_GUARD.lock().unwrap();

    let tmp = TempDir::new().unwrap();
    let riku_root = tmp.path().join(".riku");
    install_real_notify_plugin(&riku_root);

    let (port, received) = one_shot_webhook_listener();
    std::env::set_var("RIKU_ROOT", &riku_root);
    std::env::set_var(
        "RIKU_NOTIFY_WEBHOOK_URL",
        format!("http://127.0.0.1:{port}/incident"),
    );

    let log_path = tmp.path().join("test.log");
    let mut config = minimal_config(
        "sh -c 'exit 7'",
        tmp.path().to_str().unwrap(),
        log_path.to_str().unwrap(),
    );
    config.options.max_restarts = 0;

    let mut pm = ProcessManager::new().expect("ProcessManager::new should succeed");
    pm.spawn_process(&config).expect("spawn should succeed");

    // Poll the real production entry point until the crash is detected
    // and the process is permanently removed (max_restarts exceeded).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        pm.check_processes()
            .expect("check_processes should not error");
        if !pm.processes.contains_key("testapp-web-1") {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "crash was never detected and the process was never given up on"
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    // Same reasoning as the sibling test above: keep the env vars alive
    // until the webhook has actually been observed.
    let request = received
        .join()
        .expect("webhook listener thread should not panic")
        .expect("plugin never delivered the webhook within the deadline");

    std::env::remove_var("RIKU_NOTIFY_WEBHOOK_URL");
    std::env::remove_var("RIKU_ROOT");

    assert!(request.contains("POST /incident"), "got: {request}");
    assert!(
        request.contains("\"event\":\"app.failed\""),
        "got: {request}"
    );
    assert!(request.contains("\"app\":\"testapp\""), "got: {request}");
    assert!(
        request.contains("\"instance\":\"testapp-web-1\""),
        "got: {request}"
    );
    assert!(request.contains("\"exit_code\":7"), "got: {request}");
    assert!(request.contains("\"restart_count\":0"), "got: {request}");
    assert!(request.contains("\"max_restarts\":0"), "got: {request}");
}
