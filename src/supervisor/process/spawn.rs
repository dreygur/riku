//! Process spawning logic for the ProcessManager.

use anyhow::Result;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use nix::unistd::{Gid, Uid};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread;

use crate::supervisor::config::WorkerConfig;

use super::ProcessManager;

impl ProcessManager {
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
            tracing::info!("Process {} already exists, stopping it first", process_id);
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

        // Clone resource limits for use in pre_exec closure
        let limits = self.resource_limits.clone();

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

                    // Apply configured resource limits
                    limits.apply()?;

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
                                tracing::debug!("Error reading stdout: {}", e);
                                break;
                            }
                        }
                    }
                    // Explicitly drop the file handle to ensure it's closed
                    drop(stdout_log);
                    tracing::debug!("stdout log capture thread exited");
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
                                    tracing::debug!("Error reading stderr: {}", e);
                                    break;
                                }
                            }
                        }
                        // Explicitly drop the file handle to ensure it's closed
                        drop(stderr_log);
                        tracing::debug!("stderr log capture thread exited");
                    });
                }
            }
        }

        // Save PID before transferring ownership to SpawnedProcess::new().
        // This allows us to kill the child if new() fails, preventing zombie processes.
        let child_pid = child.id();

        // Create the SpawnedProcess wrapper.
        // If this fails, kill the child to prevent orphaned processes.
        let spawned_process: super::SpawnedProcess =
            match super::SpawnedProcess::new(child, config.clone(), log_handles) {
                Ok(sp) => sp,
                Err(e) => {
                    // Kill the child process using the saved PID
                    let pid = Pid::from_raw(child_pid as i32);
                    let _ = kill(pid, Signal::SIGKILL);
                    tracing::error!(
                        "Failed to create SpawnedProcess, killed child PID {}: {}",
                        child_pid, e
                    );
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

        tracing::info!("Spawned process: {} (PID: {})", process_id, pid);
        Ok(())
    }

    /// Open log files for stdout and stderr.
    pub(super) fn open_log_files(log_path: &str) -> Result<Option<(File, File)>> {
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

}
