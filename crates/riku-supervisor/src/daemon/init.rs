//! Supervisor initialization — struct definition, constructor, and PID file management.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use threadpool::ThreadPool;

use crate::cron::CronScheduler;
use crate::log_rotation::{LogRotationConfig, LogRotator};
use crate::process::ProcessManager;

use super::Supervisor;

impl Supervisor {
    /// Create a new supervisor instance.
    pub fn new(config_dir: std::path::PathBuf) -> Result<Self> {
        // Determine log root from config_dir (go up one level to .riku, then into logs)
        let log_root = config_dir
            .parent()
            .map(|p| p.join("logs"))
            .unwrap_or_else(|| std::path::PathBuf::from("./logs"));

        // Determine stats file path (in .riku root)
        let riku_root = config_dir
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let stats_file = riku_root.join("stats.json");
        let pid_file = riku_root.join("supervisor.pid");
        let control_token_file = riku_root.join("control.token");

        Ok(Supervisor {
            config_dir,
            process_manager: ProcessManager::new()?,
            watched_configs: HashMap::new(),
            log_rotator: LogRotator::new(LogRotationConfig::default()),
            log_root,
            last_log_rotation: std::time::SystemTime::now(),
            log_rotation_interval: Duration::from_secs(300),
            stats_file,
            pid_file,
            control_token_file,
            last_stats_write: std::time::SystemTime::now(),
            stats_write_interval: Duration::from_secs(5),
            cron_scheduler: CronScheduler::new(),
            last_cron_check: std::time::SystemTime::now(),
            cron_check_interval: Duration::from_secs(10),
            start_time: std::time::SystemTime::now(),
            health_running: Arc::new(AtomicBool::new(true)),
            cron_thread_pool: ThreadPool::new(10),
            pid_file_lock: None,
            metrics_broadcast_tx: None,
            actions: std::sync::Arc::new(crate::health::NoopControlActions),
        })
    }

    /// Inject the control-plane action implementation (provided by the binary).
    /// Without this, control routes report the control plane as unavailable.
    pub fn with_actions(mut self, actions: crate::health::SharedActions) -> Self {
        self.actions = actions;
        self
    }

    /// Create PID file with exclusive lock to prevent multiple supervisors.
    ///
    /// Returns Ok(File) with the locked file handle (lock is held until File is dropped).
    /// Returns Err if another supervisor is already running.
    pub(super) fn create_pid_file_with_lock(&self, pid: u32) -> Result<fs::File> {
        use std::fs::OpenOptions;
        use std::io::Write;

        // Create or open PID file
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.pid_file)?;

        // Try to acquire exclusive lock (non-blocking)
        #[cfg(unix)]
        {
            use nix::libc;
            use std::os::unix::io::AsRawFd;

            // Use libc::flock directly (portable across Unix systems)
            let fd = file.as_raw_fd();
            let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };

            if result != 0 {
                return Err(anyhow::anyhow!(
                    "Failed to lock PID file (another supervisor running?): {}",
                    std::io::Error::last_os_error()
                ));
            }

            // Lock is held until file descriptor is closed (when File is dropped)
        }

        // Write PID to file
        writeln!(file, "{}", pid)?;
        file.flush()?;

        Ok(file)
    }
}
