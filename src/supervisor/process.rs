//! Process management module for the supervisor.
//!
//! Handles spawning, monitoring, health checks, and managing application processes.

use anyhow::Result;
use nix::sys::resource::{setrlimit, Resource};
use nix::sys::signal::{kill, Signal};
use nix::unistd::{Gid, Pid, Uid};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::supervisor::config::{HealthCheck, WorkerConfig};
use crate::supervisor::stats::{get_process_resources, HealthStatus, ProcessStatus, StatsManager};

/// Represents a spawned application process with metadata.
pub struct SpawnedProcess {
    pub pid: Pid,
    pub child: Child,
    pub config: WorkerConfig,
    pub restart_count: u32,
    pub last_restart: std::time::Instant,
    pub health_check_config: Option<HealthCheck>,
    pub consecutive_health_failures: u32,
    #[allow(dead_code)]
    log_handles: Option<(File, File)>,
}

impl SpawnedProcess {
    /// Create a new SpawnedProcess instance.
    pub fn new(
        child: Child,
        config: WorkerConfig,
        log_handles: Option<(File, File)>,
    ) -> Result<Self> {
        let pid = Pid::from_raw(child.id() as i32);
        let health_check_config = config.options.health_check.clone();

        Ok(SpawnedProcess {
            pid,
            child,
            config,
            restart_count: 0,
            last_restart: std::time::Instant::now(),
            health_check_config,
            consecutive_health_failures: 0,
            log_handles,
        })
    }

    /// Check if the process is still running.
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(_status)) => false, // Process has exited
            Ok(None) => true,           // Process is still running
            Err(_) => false,            // Error checking status, assume dead
        }
    }

    /// Send a termination signal to the process.
    pub fn terminate(&mut self) -> Result<()> {
        kill(self.pid, Signal::SIGTERM)?;
        Ok(())
    }

    /// Force kill the process.
    pub fn kill(&mut self) -> Result<()> {
        kill(self.pid, Signal::SIGKILL)?;
        Ok(())
    }

    /// Get the process ID as u32.
    pub fn pid_as_u32(&self) -> u32 {
        self.pid.as_raw() as u32
    }
}

impl Drop for SpawnedProcess {
    fn drop(&mut self) {
        // Ensure the child process is cleaned up when SpawnedProcess is dropped.
        // Child::drop does NOT kill the process, so we must do it explicitly.
        if self.is_running() {
            let _ = self.terminate();
            // Give brief time for graceful shutdown
            thread::sleep(Duration::from_millis(100));
            if self.is_running() {
                let _ = self.kill();
            }
        }
        // Reap the child to avoid zombies
        let _ = self.child.wait();
    }
}

/// Manages the lifecycle of application processes.
pub struct ProcessManager {
    processes: HashMap<String, SpawnedProcess>, // Key: app_name-worker_kind-ordinal
    stats: StatsManager,
}

impl ProcessManager {
    /// Create a new process manager.
    pub fn new() -> Result<Self> {
        Ok(ProcessManager {
            processes: HashMap::new(),
            stats: StatsManager::new(),
        })
    }

    /// Get the number of managed processes.
    pub fn get_process_count(&self) -> usize {
        self.processes.len()
    }

    /// Get a reference to the stats manager.
    #[allow(dead_code)]
    pub fn stats(&self) -> &StatsManager {
        &self.stats
    }

    /// Get a mutable reference to the stats manager.
    #[allow(dead_code)]
    pub fn stats_mut(&mut self) -> &mut StatsManager {
        &mut self.stats
    }

    /// Spawn a new process based on the worker configuration.
    pub fn spawn_process(&mut self, config: &WorkerConfig) -> Result<()> {
        use std::os::unix::process::CommandExt;

        let app_name = &config.worker.app;
        let worker_kind = &config.worker.kind;
        let ordinal = config.worker.ordinal;

        // Create a unique identifier for this process
        let process_id = format!("{}-{}-{}", app_name, worker_kind, ordinal);

        // Check if process already exists
        if self.processes.contains_key(&process_id) {
            println!("Process {} already exists, stopping it first", process_id);
            self.stop_process_by_id(&process_id)?;
        }

        // Open log files for stdout and stderr
        let log_path = &config.options.log_file;
        let log_handles = Self::open_log_files(log_path)?;

        // Resolve optional uid/gid names to numeric IDs before forking.
        // This must happen in the parent so we can use the libc name-lookup functions safely.
        let target_uid: Option<Uid> = config.options.uid.as_deref().and_then(|name| {
            // Try numeric first, then name lookup via nix
            if let Ok(n) = name.parse::<u32>() {
                return Some(Uid::from_raw(n));
            }
            // nix::unistd::User::from_name uses getpwnam
            nix::unistd::User::from_name(name)
                .ok()
                .flatten()
                .map(|u| u.uid)
        });
        let target_gid: Option<Gid> = config.options.gid.as_deref().and_then(|name| {
            if let Ok(n) = name.parse::<u32>() {
                return Some(Gid::from_raw(n));
            }
            nix::unistd::Group::from_name(name)
                .ok()
                .flatten()
                .map(|g| g.gid)
        });

        // Build the command to run
        let mut cmd = Command::new("sh");
        // Set resource limits to prevent runaway processes (pre_exec is unsafe)
        unsafe {
            cmd.arg("-c")
                .arg(&config.worker.command)
                .current_dir(&config.options.working_dir)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                // Create new process group for proper signal handling
                .process_group(0)
                // Set resource limits in child process before exec
                .pre_exec(move || {
                    // Drop to configured gid/uid if specified (gid must be set before uid).
                    if let Some(gid) = target_gid {
                        nix::unistd::setgid(gid).map_err(|e| {
                            std::io::Error::new(std::io::ErrorKind::PermissionDenied, e.to_string())
                        })?;
                    }
                    if let Some(uid) = target_uid {
                        nix::unistd::setuid(uid).map_err(|e| {
                            std::io::Error::new(std::io::ErrorKind::PermissionDenied, e.to_string())
                        })?;
                    }
                    // Limit max open files to 1024 (prevents fd exhaustion)
                    let _ = setrlimit(Resource::RLIMIT_NOFILE, 1024, 1024);
                    // Limit max processes to 64 (prevents fork bombs)
                    let _ = setrlimit(Resource::RLIMIT_NPROC, 64, 64);
                    Ok(())
                });
        }

        // Set environment variables
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        // Spawn the process
        let mut child = cmd.spawn()?;

        // Start log capture threads before creating SpawnedProcess
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        if let Some((ref log_file, ref _log_file_mut)) = log_handles {
            // Capture stdout
            if let Some(stdout_reader) = stdout {
                let mut stdout_log = log_file.try_clone()?;
                thread::spawn(move || {
                    let reader = BufReader::new(stdout_reader);
                    for line in reader.lines() {
                        match line {
                            Ok(line) => {
                                let _ = writeln!(stdout_log, "{}", line);
                                let _ = stdout_log.flush();
                            }
                            Err(e) => {
                                eprintln!("Error reading stdout: {}", e);
                                break;
                            }
                        }
                    }
                });
            }

            // Capture stderr
            if let Some(stderr_reader) = stderr {
                if let Some((_, ref stderr_log)) = log_handles {
                    let mut stderr_log = stderr_log.try_clone()?;
                    thread::spawn(move || {
                        let reader = BufReader::new(stderr_reader);
                        for line in reader.lines() {
                            match line {
                                Ok(line) => {
                                    let _ = writeln!(stderr_log, "{}", line);
                                    let _ = stderr_log.flush();
                                }
                                Err(e) => {
                                    eprintln!("Error reading stderr: {}", e);
                                    break;
                                }
                            }
                        }
                    });
                }
            }
        }

        // Create the SpawnedProcess wrapper.
        // If this fails, kill the child to prevent orphaned processes.
        let spawned_process = match SpawnedProcess::new(child, config.clone(), log_handles) {
            Ok(sp) => sp,
            Err(e) => {
                // SpawnedProcess::new takes ownership of child, but on error
                // we can't access it. In practice this path is unreachable since
                // new() is infallible, but defend against future changes.
                return Err(e);
            }
        };
        let pid = spawned_process.pid_as_u32();

        // Register in stats
        self.stats.register_process(
            process_id.clone(),
            app_name.clone(),
            worker_kind.clone(),
            ordinal,
        );

        // Store the process
        self.processes.insert(process_id.clone(), spawned_process);

        println!("Spawned process: {} (PID: {})", process_id, pid);
        Ok(())
    }

    /// Open log files for stdout and stderr.
    fn open_log_files(log_path: &str) -> Result<Option<(File, File)>> {
        use std::path::Path;

        let path = Path::new(log_path);

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Open log file for appending
        let log_file = OpenOptions::new().create(true).append(true).open(path)?;

        // Return two handles (one for stdout, one for stderr - both write to same file)
        let stdout_handle = log_file.try_clone()?;
        let stderr_handle = log_file.try_clone()?;

        Ok(Some((stdout_handle, stderr_handle)))
    }

    /// Stop a specific process by its ID.
    fn stop_process_by_id(&mut self, process_id: &str) -> Result<()> {
        if let Some(mut process) = self.processes.remove(process_id) {
            println!("Stopping process: {} (PID: {})", process_id, process.pid);

            // Update stats
            self.stats.mark_stopped(process_id);

            // Try graceful shutdown with SIGTERM
            process.terminate()?;

            // Wait for graceful shutdown using the configured grace_period (in seconds).
            // Poll every 100 ms so we exit promptly when the process dies.
            let grace_period = process.config.options.grace_period;
            let deadline = Duration::from_secs(grace_period);
            let poll_interval = Duration::from_millis(100);
            let mut elapsed = Duration::ZERO;
            while process.is_running() && elapsed < deadline {
                thread::sleep(poll_interval);
                elapsed += poll_interval;
            }

            // If still running after the grace period, force kill with SIGKILL
            if process.is_running() {
                println!(
                    "Process {} didn't respond to SIGTERM within {}s, sending SIGKILL",
                    process_id, grace_period
                );
                process.kill()?;

                thread::sleep(Duration::from_millis(500));
            }

            println!("Process {} stopped", process_id);
        }
        Ok(())
    }

    /// Stop all processes for a specific app.
    pub fn stop_app_processes(&mut self, app_name: &str) -> Result<()> {
        let processes_to_remove: Vec<String> = self
            .processes
            .keys()
            .filter(|id| id.starts_with(&format!("{}-", app_name)))
            .cloned()
            .collect();

        for process_id in processes_to_remove {
            self.stop_process_by_id(&process_id)?;
        }
        Ok(())
    }

    /// Check the status of all managed processes, perform health checks, and restart crashed ones.
    pub fn check_processes(&mut self) -> Result<()> {
        let mut to_restart = Vec::new();
        let mut health_checks: Vec<(String, HealthCheck)> = Vec::new();

        // First pass: check processes and collect health check configs
        for (process_id, process) in self.processes.iter_mut() {
            // Check if process is still running
            if !process.is_running() {
                println!("Process {} has crashed", process_id);
                self.stats.mark_crashed(process_id);

                // Enforce max_restarts: stop trying once the limit is hit.
                let max_restarts = process.config.options.max_restarts;
                if process.restart_count >= max_restarts {
                    println!(
                        "Process {} has crashed {} time(s) (max_restarts={}), giving up",
                        process_id, process.restart_count, max_restarts
                    );
                    // Mark as failed in stats and queue for removal so the dead child
                    // entry is dropped (reaping the zombie) and stops polluting logs.
                    self.stats.mark_crashed(process_id);
                    to_restart.push(format!("__remove__{}", process_id));
                    continue;
                }

                // Calculate backoff time based on restart count
                let backoff = std::cmp::min(60, 2_i32.pow(process.restart_count.min(6))) as u64;

                // Only restart if enough time has passed since the last restart
                if process.last_restart.elapsed().as_secs() >= backoff {
                    to_restart.push(process_id.clone());
                }
                continue;
            }

            // Process is running - update stats
            self.stats.mark_running(process_id, process.pid_as_u32());

            // Update resource usage
            if let Some((cpu_ms, memory)) = get_process_resources(process.pid_as_u32()) {
                self.stats.update_resource_usage(process_id, cpu_ms, memory);
            }

            // Collect health check configs for later
            if let Some(health_config) = &process.health_check_config {
                health_checks.push((process_id.clone(), health_config.clone()));
            }
        }

        // Second pass: perform health checks (avoids borrow checker issues)
        for (process_id, health_config) in health_checks {
            let health_status = self.perform_health_check(&process_id, &health_config);

            // Update stats with health check result
            self.stats
                .update_health_check(&process_id, health_status.clone());

            // Update consecutive failures
            if let Some(process) = self.processes.get_mut(&process_id) {
                match health_status {
                    HealthStatus::Healthy => {
                        process.consecutive_health_failures = 0;
                    }
                    _ => {
                        println!(
                            "Health check for {} failed: {:?}",
                            process_id, health_status
                        );
                        process.consecutive_health_failures += 1;

                        // Restart if too many consecutive failures
                        if process.consecutive_health_failures >= health_config.retries {
                            println!(
                                "Process {} failed {} consecutive health checks, restarting",
                                process_id, process.consecutive_health_failures
                            );
                            to_restart.push(process_id);
                        }
                    }
                }
            }
        }

        // Restart processes that need it; entries prefixed "__remove__" are
        // permanently failed processes that must be removed without restarting.
        for process_id in to_restart {
            if let Some(id) = process_id.strip_prefix("__remove__") {
                // Remove the dead entry so Drop reaps the zombie child.
                self.processes.remove(id);
                eprintln!(
                    "Process {} permanently failed; removed from supervision",
                    id
                );
            } else {
                self.restart_process(&process_id)?;
            }
        }

        Ok(())
    }

    /// Perform an HTTP health check on a process.
    fn perform_health_check(&self, process_id: &str, config: &HealthCheck) -> HealthStatus {
        use reqwest::blocking::Client;

        let port = self.get_process_port(process_id);
        let url = match port {
            Some(p) => format!("http://127.0.0.1:{}{}", p, config.url),
            None => return HealthStatus::Error("No port configured".to_string()),
        };

        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout))
            .build()
            .unwrap_or_else(|_| Client::new());

        match client.get(&url).send() {
            Ok(response) => {
                if response.status().is_success() {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Unhealthy
                }
            }
            Err(e) => {
                if e.is_timeout() {
                    HealthStatus::Timeout
                } else {
                    HealthStatus::Error(e.to_string())
                }
            }
        }
    }

    /// Get the port for a process from its environment.
    fn get_process_port(&self, process_id: &str) -> Option<u16> {
        self.processes
            .get(process_id)
            .and_then(|p| p.config.env.get("PORT"))
            .and_then(|p| p.parse::<u16>().ok())
    }

    /// Restart a specific process.
    fn restart_process(&mut self, process_id: &str) -> Result<()> {
        println!("Restarting process: {}", process_id);

        // Capture config and current restart count before removing the process.
        let (config, prev_restart_count) = match self.processes.get(process_id) {
            Some(p) => (Some(p.config.clone()), p.restart_count),
            None => (None, 0),
        };

        if let Some(config) = config {
            // Mark as restarting in stats
            self.stats.mark_restarting(process_id);

            // Remove the old process entry (terminates it via Drop).
            self.processes.remove(process_id);

            // Spawn a fresh process and then update its restart counter so that
            // the exponential backoff keeps growing across successive crashes.
            self.spawn_process(&config)?;

            if let Some(new_process) = self.processes.get_mut(process_id) {
                new_process.restart_count = prev_restart_count + 1;
                new_process.last_restart = std::time::Instant::now();
            }
        }

        Ok(())
    }

    /// Hot reload a process - graceful restart with zero downtime.
    ///
    /// Spawns the new process under a temporary key, gives it time to start,
    /// then gracefully shuts down the old process and renames the new entry
    /// to the canonical process_id. This avoids the race where spawn_process()
    /// would stop the old process automatically when the key already exists.
    #[allow(dead_code)]
    pub fn hot_reload_process(&mut self, process_id: &str) -> Result<()> {
        let (config, old_pid) = match self.processes.get(process_id) {
            Some(p) => (p.config.clone(), p.pid_as_u32()),
            None => return Ok(()),
        };

        println!("Hot reloading process {} (PID: {})", process_id, old_pid);

        // Spawn new process under a temporary key so spawn_process() does not
        // automatically stop the old entry (which lives under process_id).
        let temp_id = format!("{}__hot_new", process_id);
        let mut new_config = config.clone();
        // Temporarily change the ordinal to produce a different process_id key.
        // We will rename it back after the old process is stopped.
        new_config.worker.ordinal = new_config.worker.ordinal.wrapping_add(u32::MAX / 2);
        self.spawn_process(&new_config)?;
        let new_temp_key = format!(
            "{}-{}-{}",
            new_config.worker.app, new_config.worker.kind, new_config.worker.ordinal
        );

        // Give the new process time to start and become ready.
        thread::sleep(Duration::from_millis(500));

        // Remove the old process entry (Drop triggers SIGTERM → wait → SIGKILL).
        if let Some(mut old_process) = self.processes.remove(process_id) {
            let grace = old_process.config.options.grace_period;
            old_process.terminate()?;
            let deadline = Duration::from_secs(grace);
            let poll = Duration::from_millis(100);
            let mut elapsed = Duration::ZERO;
            while old_process.is_running() && elapsed < deadline {
                thread::sleep(poll);
                elapsed += poll;
            }
            if old_process.is_running() {
                old_process.kill()?;
                thread::sleep(Duration::from_millis(100));
            }
        }

        // Rename the new process entry from the temp key back to the canonical key.
        if let Some(mut new_process) = self.processes.remove(&new_temp_key) {
            // Restore the original ordinal in the config so stats and future
            // operations use the correct process_id.
            new_process.config.worker.ordinal = config.worker.ordinal;
            self.processes.insert(process_id.to_string(), new_process);
        }

        // Also clean up the temp_id entry if it was inserted under yet another key.
        self.processes.remove(&temp_id);

        println!("Hot reload complete for {}", process_id);
        Ok(())
    }

    /// Hot reload all processes for an app.
    #[allow(dead_code)]
    pub fn hot_reload_app(&mut self, app_name: &str) -> Result<()> {
        let process_ids: Vec<String> = self
            .processes
            .keys()
            .filter(|id| id.starts_with(&format!("{}-", app_name)))
            .cloned()
            .collect();

        for process_id in process_ids {
            self.hot_reload_process(&process_id)?;
        }

        Ok(())
    }

    /// Stop all managed processes, respecting each process's configured grace_period.
    pub fn stop_all_processes(&mut self) -> Result<()> {
        let process_ids: Vec<String> = self.processes.keys().cloned().collect();

        for process_id in process_ids {
            self.stop_process_by_id(&process_id)?;
        }
        Ok(())
    }

    /// Get a list of all managed processes with their status.
    #[allow(dead_code)]
    pub fn list_processes(&self) -> Vec<ProcessInfo> {
        self.processes
            .iter()
            .map(|(id, process)| {
                let stats = self.stats.get_process_stats(id);
                ProcessInfo {
                    process_id: id.clone(),
                    app: process.config.worker.app.clone(),
                    kind: process.config.worker.kind.clone(),
                    ordinal: process.config.worker.ordinal,
                    pid: process.pid_as_u32(),
                    status: stats
                        .map(|s| s.status.clone())
                        .unwrap_or(ProcessStatus::Running),
                    restart_count: process.restart_count,
                }
            })
            .collect()
    }

    /// Get processes for a specific app.
    #[allow(dead_code)]
    pub fn get_app_processes(&self, app_name: &str) -> Vec<ProcessInfo> {
        self.list_processes()
            .into_iter()
            .filter(|p| p.app == app_name)
            .collect()
    }
}

/// Process information for CLI display.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub process_id: String,
    pub app: String,
    pub kind: String,
    pub ordinal: u32,
    pub pid: u32,
    pub status: ProcessStatus,
    pub restart_count: u32,
}
