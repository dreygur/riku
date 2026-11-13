//! A stand in for the riku supervisor in deploy tests: it listens on the
//! port of every enabled worker config, so a deploy's health check on a new
//! web copy passes, and closes it once the config is gone.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use riku::supervisor::config::WorkerConfig;

const POLL_INTERVAL: Duration = Duration::from_millis(10);

fn enabled_ports(dir: &Path) -> Vec<u16> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .filter_map(|path| {
            let text = fs::read_to_string(&path).ok();
            let config: Option<WorkerConfig> = text.and_then(|text| toml::from_str(&text).ok());
            config.and_then(|config| config.env.get("PORT").and_then(|port| port.parse().ok()))
        })
        .collect()
}

/// Keyed by port, not file: a slot freed and taken again between two polls
/// gets the new copy's port.
fn supervise(dir: PathBuf, running: Arc<AtomicBool>) {
    let mut listeners: HashMap<u16, TcpListener> = HashMap::new();
    while running.load(Ordering::SeqCst) {
        let enabled = enabled_ports(&dir);
        listeners.retain(|port, _| enabled.contains(port));
        for port in enabled {
            if listeners.contains_key(&port) {
                continue;
            }
            if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) {
                listeners.insert(port, listener);
            }
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Writes `supervisor.pid` and holds its lock, the way a running supervisor
/// does, so a deploy finds one running and never launches a real one.
///
/// The pid is a child `sleep`, not this test process: a deploy sends SIGHUP
/// to that pid, which would kill the test runner.
fn hold_pid_file(paths: &riku::config::RikuPaths) -> (File, Child) {
    fs::create_dir_all(&paths.riku_root).expect("create the riku root");
    let stand_in = Command::new("sleep")
        .arg("600")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn the pid stand in");
    let mut pid_file =
        File::create(paths.riku_root.join("supervisor.pid")).expect("create supervisor.pid");
    pid_file.lock().expect("lock supervisor.pid");
    writeln!(pid_file, "{}", stand_in.id()).expect("write supervisor.pid");
    (pid_file, stand_in)
}

/// Runs until dropped.
pub struct MockSupervisor {
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    /// Its lock is what makes the supervisor look running; it is released
    /// when the mock is dropped.
    _pid_file: File,
    stand_in: Child,
}

impl MockSupervisor {
    pub fn start(paths: &riku::config::RikuPaths) -> MockSupervisor {
        let (pid_file, stand_in) = hold_pid_file(paths);
        let running = Arc::new(AtomicBool::new(true));
        let dir = paths.workers_enabled.clone();
        let thread_running = Arc::clone(&running);
        let handle = thread::spawn(move || supervise(dir, thread_running));
        MockSupervisor {
            running,
            handle: Some(handle),
            _pid_file: pid_file,
            stand_in,
        }
    }
}

impl Drop for MockSupervisor {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        let _ = self.stand_in.kill();
        let _ = self.stand_in.wait();
    }
}
