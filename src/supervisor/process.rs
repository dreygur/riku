//! Process management module for the supervisor.
//!
//! Handles spawning, monitoring, and managing application processes.

use anyhow::Result;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::supervisor::config::WorkerConfig;

/// Represents a spawned application process with metadata.
pub struct SpawnedProcess {
    pub pid: Pid,
    pub child: Child,
    pub config: WorkerConfig,
    pub restart_count: u32,
    pub last_restart: std::time::Instant,
}

impl SpawnedProcess {
    /// Create a new SpawnedProcess instance.
    pub fn new(child: Child, config: WorkerConfig) -> Result<Self> {
        let pid = Pid::from_raw(child.id() as i32);
        Ok(SpawnedProcess {
            pid,
            child,
            config,
            restart_count: 0,
            last_restart: std::time::Instant::now(),
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
}

/// Manages the lifecycle of application processes.
pub struct ProcessManager {
    processes: HashMap<String, SpawnedProcess>, // Key: app_name-worker_kind-ordinal
}

impl ProcessManager {
    /// Create a new process manager.
    pub fn new() -> Result<Self> {
        Ok(ProcessManager {
            processes: HashMap::new(),
        })
    }

    /// Spawn a new process based on the worker configuration.
    pub fn spawn_process(&mut self, config: &WorkerConfig) -> Result<()> {
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
            .stderr(Stdio::piped());

        // Set environment variables
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        // Spawn the process
        let child = cmd.spawn()?;

        // Create the SpawnedProcess wrapper
        let spawned_process = SpawnedProcess::new(child, config.clone())?;
        let pid = spawned_process.pid;

        // Store the process
        self.processes.insert(process_id.clone(), spawned_process);

        println!("Spawned process: {} (PID: {})", process_id, pid);
        Ok(())
    }

    /// Stop a specific process by its ID.
    fn stop_process_by_id(&mut self, process_id: &str) -> Result<()> {
        if let Some(mut process) = self.processes.remove(process_id) {
            println!("Stopping process: {} (PID: {})", process_id, process.pid);

            // Try graceful shutdown with SIGTERM
            process.terminate()?;

            // Wait for graceful shutdown (with timeout)
            let mut attempts = 0;
            while process.is_running() && attempts < 10 {
                // Wait up to 10 seconds
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

                // Wait a bit more to ensure it's gone
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

    /// Check the status of all managed processes and restart crashed ones.
    pub fn check_processes(&mut self) -> Result<()> {
        let mut to_restart = Vec::new();

        for (process_id, process) in self.processes.iter_mut() {
            if !process.is_running() {
                println!("Process {} has crashed", process_id);

                // Calculate backoff time based on restart count
                let backoff = std::cmp::min(60, 2_i32.pow(process.restart_count.min(6))) as u64;

                // Only restart if enough time has passed since the last restart
                if process.last_restart.elapsed().as_secs() >= backoff {
                    to_restart.push(process_id.clone());
                }
            }
        }

        for process_id in to_restart {
            println!("Restarting process: {}", process_id);
            // Get the config before removing the process
            let config = self.processes.get(&process_id).map(|p| p.config.clone());

            if let Some(config) = config {
                // Remove the old process
                if let Some(mut process) = self.processes.remove(&process_id) {
                    process.restart_count += 1;
                    process.last_restart = std::time::Instant::now();

                    // Respawn the process with the original config
                    // Don't put the old process back, just respawn
                    self.spawn_process(&config)?;
                }
            }
        }

        Ok(())
    }

    /// Stop all managed processes.
    pub fn stop_all_processes(&mut self) -> Result<()> {
        let process_ids: Vec<String> = self.processes.keys().cloned().collect();

        for process_id in process_ids {
            println!("Stopping process: {}", process_id);
            if let Some(mut process) = self.processes.remove(&process_id) {
                // Try graceful shutdown with SIGTERM
                process.terminate()?;

                // Wait for graceful shutdown (with timeout)
                let mut attempts = 0;
                while process.is_running() && attempts < 10 {
                    // Wait up to 10 seconds
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                    attempts += 1;
                }

                // If still running, force kill with SIGKILL
                if process.is_running() {
                    println!(
                        "Process {} didn't respond to SIGTERM, sending SIGKILL",
                        process_id
                    );
                    process.kill()?;

                    // Wait a bit more to ensure it's gone
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }

                println!("Process {} stopped", process_id);
            }
        }
        Ok(())
    }

    /// Get a list of all managed processes.
    #[allow(dead_code)]
    pub fn list_processes(&self) -> Vec<String> {
        self.processes.keys().cloned().collect()
    }
}
