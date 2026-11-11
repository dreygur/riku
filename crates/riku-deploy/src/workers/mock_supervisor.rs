//! Test doubles for a Procfile deploy: a supervisor that "runs" each enabled
//! web copy by listening where it would, and a route sink that records what
//! the router would have been given.

use std::collections::HashMap;
use std::fs;
use std::net::TcpListener;
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::Result;
use riku_supervisor::config::WorkerConfig;
use riku_util::backend::Backend;

use super::web_env::is_web_facing;
use crate::config::RikuPaths;
use crate::rolling::RouteSink;

const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Every enabled worker config that parses, with its path.
fn enabled_configs(dir: &Path) -> Vec<(PathBuf, WorkerConfig)> {
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
            text.and_then(|text| toml::from_str(&text).ok())
                .map(|config| (path, config))
        })
        .collect()
}

fn web_count(configs: &[(PathBuf, WorkerConfig)]) -> usize {
    configs
        .iter()
        .filter(|(_, config)| is_web_facing(&config.worker.kind))
        .count()
}

/// Where the copy `config` describes listens: its `PORT`, else its uWSGI
/// socket.
fn address(config: &WorkerConfig) -> Option<&String> {
    config
        .env
        .get("PORT")
        .or_else(|| config.env.get("UWSGI_SOCKET"))
}

/// Listens on `address`. The listener stays open as long as the fd lives. A
/// socket gets the `0660` mode uWSGI's `chmod-socket` gives it, and is left
/// in place on close, as uWSGI leaves it.
fn listen_on(address: &str) -> Option<OwnedFd> {
    if let Ok(port) = address.parse::<u16>() {
        return TcpListener::bind(("127.0.0.1", port))
            .ok()
            .map(OwnedFd::from);
    }
    let _ = fs::remove_file(address);
    let listener = UnixListener::bind(address).ok();
    let _ = fs::set_permissions(address, fs::Permissions::from_mode(0o660));
    listener.map(OwnedFd::from)
}

/// Records each published backend list with the number of web copies that
/// were enabled at that moment.
pub(crate) struct MockRoutes {
    pub published: Vec<(Vec<Backend>, usize)>,
    /// Makes `check` fail, as a router helper that is down does.
    pub refuse_check: bool,
    /// The 1-based publish call that fails.
    fail_publish: Option<usize>,
    publishes: usize,
    paths: RikuPaths,
}

impl MockRoutes {
    pub fn new(paths: &RikuPaths) -> MockRoutes {
        MockRoutes {
            published: Vec::new(),
            refuse_check: false,
            fail_publish: None,
            publishes: 0,
            paths: paths.clone(),
        }
    }

    /// Fails the `nth` publish after this call.
    pub fn fail_publish_after(&mut self, nth: usize) {
        self.fail_publish = Some(self.publishes + nth);
    }
}

impl RouteSink for MockRoutes {
    fn publish(&mut self, backends: &[Backend]) -> Result<()> {
        self.publishes += 1;
        let enabled = web_count(&enabled_configs(&self.paths.workers_enabled));
        self.published.push((backends.to_vec(), enabled));
        if self.fail_publish == Some(self.publishes) {
            return Err(anyhow::anyhow!("reload not confirmed"));
        }
        Ok(())
    }

    fn check(&mut self) -> Result<()> {
        if self.refuse_check {
            return Err(anyhow::anyhow!("router helper not reachable"));
        }
        Ok(())
    }
}

/// Polls `workers_enabled` and keeps a listener open on the address of every
/// enabled copy, closing it once no enabled config names that address. Keyed
/// by address, not file: a slot freed and taken again between two polls gets
/// the new copy's address. Stops when dropped.
pub(crate) struct MockSupervisor {
    running: Arc<AtomicBool>,
    max_web: Arc<AtomicUsize>,
    handle: Option<JoinHandle<()>>,
}

fn supervise(dir: PathBuf, running: Arc<AtomicBool>, max_web: Arc<AtomicUsize>) {
    let mut listeners: HashMap<String, OwnedFd> = HashMap::new();
    while running.load(Ordering::SeqCst) {
        let configs = enabled_configs(&dir);
        max_web.fetch_max(web_count(&configs), Ordering::SeqCst);
        let addresses: Vec<&String> = configs.iter().filter_map(|(_, c)| address(c)).collect();
        listeners.retain(|address, _| addresses.contains(&address));
        for address in addresses {
            if listeners.contains_key(address) {
                continue;
            }
            if let Some(listener) = listen_on(address) {
                listeners.insert(address.clone(), listener);
            }
        }
        thread::sleep(POLL_INTERVAL);
    }
}

impl MockSupervisor {
    pub fn start(paths: &RikuPaths) -> MockSupervisor {
        let running = Arc::new(AtomicBool::new(true));
        let max_web = Arc::new(AtomicUsize::new(0));
        let dir = paths.workers_enabled.clone();
        let (thread_running, thread_max) = (Arc::clone(&running), Arc::clone(&max_web));
        let handle = thread::spawn(move || supervise(dir, thread_running, thread_max));
        MockSupervisor {
            running,
            max_web,
            handle: Some(handle),
        }
    }

    /// The most web copies that were enabled at once.
    pub fn max_enabled_web(&self) -> usize {
        self.max_web.load(Ordering::SeqCst)
    }
}

impl Drop for MockSupervisor {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
