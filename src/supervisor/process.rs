//! Process management module for the supervisor.
//!
//! Handles spawning, monitoring, health checks, and managing application processes.

use anyhow::Result;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::collections::HashMap;
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
}

impl SpawnedProcess {
    /// Create a new SpawnedProcess instance.
    pub fn new(child: Child, config: WorkerConfig) -> Result<Self> {
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

        // Build the command to run
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(&config.worker.command)
            .current_dir(&config.options.working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Create new process group for proper signal handling
            .process_group(0);

        // Set environment variables
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        // Spawn the process
        let child = cmd.spawn()?;

        // Create the SpawnedProcess wrapper.
        // If this fails, kill the child to prevent orphaned processes.
        let spawned_process = match SpawnedProcess::new(child, config.clone()) {
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

    /// Stop a specific process by its ID.
    fn stop_process_by_id(&mut self, process_id: &str) -> Result<()> {
        if let Some(mut process) = self.processes.remove(process_id) {
            println!("Stopping process: {} (PID: {})", process_id, process.pid);

            // Update stats
            self.stats.mark_stopped(process_id);

            // Try graceful shutdown with SIGTERM
            process.terminate()?;

            // Wait for graceful shutdown (with timeout)
            let mut attempts = 0;
            while process.is_running() && attempts < 10 {
                thread::sleep(Duration::from_millis(1000));
                attempts += 1;
            }

            // If still running, force kill with SIGKILL
            if process.is_running() {
                println!(
                    "Process {} didn't respond to SIGTERM, sending SIGKILL",
                    process_id
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

        // Restart processes that need it
        for process_id in to_restart {
            self.restart_process(&process_id)?;
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

        // Get the config before removing the process
        let config = self.processes.get(process_id).map(|p| p.config.clone());

        if let Some(config) = config {
            // Mark as restarting in stats
            self.stats.mark_restarting(process_id);

            // Remove the old process
            if let Some(mut process) = self.processes.remove(process_id) {
                process.restart_count += 1;
                process.last_restart = std::time::Instant::now();

                // Respawn the process with the original config
                self.spawn_process(&config)?;
            }
        }

        Ok(())
    }

    /// Hot reload a process - graceful restart with zero downtime.
    #[allow(dead_code)]
    pub fn hot_reload_process(&mut self, process_id: &str) -> Result<()> {
        if let Some(process) = self.processes.get(process_id) {
            let config = process.config.clone();
            let old_pid = process.pid_as_u32();

            println!("Hot reloading process {} (PID: {})", process_id, old_pid);

            // Spawn new process first
            self.spawn_process(&config)?;

            // Give new process time to start
            thread::sleep(Duration::from_millis(500));

            // Gracefully stop old process
            if let Some(mut old_process) = self.processes.remove(process_id) {
                old_process.terminate()?;

                // Wait for graceful shutdown
                let mut attempts = 0;
                while old_process.is_running() && attempts < 30 {
                    thread::sleep(Duration::from_millis(100));
                    attempts += 1;
                }

                // Force kill if still running
                if old_process.is_running() {
                    old_process.kill()?;
                }
            }

            println!("Hot reload complete for {}", process_id);
        }

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

    /// Stop all managed processes.
    pub fn stop_all_processes(&mut self) -> Result<()> {
        let process_ids: Vec<String> = self.processes.keys().cloned().collect();

        for process_id in process_ids {
            println!("Stopping process: {}", process_id);
            if let Some(mut process) = self.processes.remove(&process_id) {
                process.terminate()?;

                let mut attempts = 0;
                while process.is_running() && attempts < 10 {
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                    attempts += 1;
                }

                if process.is_running() {
                    println!(
                        "Process {} didn't respond to SIGTERM, sending SIGKILL",
                        process_id
                    );
                    process.kill()?;
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }

                println!("Process {} stopped", process_id);
            }
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
