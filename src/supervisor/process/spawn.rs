//! Process spawning logic for the ProcessManager.

use anyhow::Result;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use nix::unistd::{Gid, Uid};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread;

use crate::supervisor::cgroups::{CgroupLimits, WorkerCgroup};
use crate::supervisor::config::WorkerConfig;

use super::isolation::NamespaceConfig;
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

        // Provision the worker's cgroup (if isolation is enabled) before
        // spawning, so the constraints already exist when the worker joins
        // it from within pre_exec.
        let cgroup: Option<WorkerCgroup> = match &config.options.isolation {
            Some(opts) => Some(WorkerCgroup::provision(
                &process_id,
                &CgroupLimits {
                    memory_max_bytes: opts.max_memory_bytes,
                    cpu_quota_us: opts.cpu_quota_us,
                    cpu_period_us: opts.cpu_period_us,
                },
            )?),
            None => None,
        };
        let cgroup_for_child = cgroup.clone();

        let namespace_config = NamespaceConfig {
            enabled: config.options.isolation.is_some(),
            isolated_root: config
                .options
                .isolation
                .as_ref()
                .map(|opts| std::path::PathBuf::from(&opts.root_dir)),
        };

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

                    // Join the cgroup using our own (real, top-level) PID
                    // before any namespace isolation below, while it still
                    // matches what the parent sees as `child.id()`.
                    if let Some(cgroup) = &cgroup_for_child {
                        cgroup.add_self()?;
                    }

                    // Apply configured resource limits
                    limits.apply()?;

                    // Namespace isolation runs last: on success for the
                    // PID-namespace branch it either returns Ok(()) in the
                    // process that's about to exec, or never returns at all
                    // (the outer process became a signal-forwarding shim
                    // and already called _exit). See isolation.rs.
                    namespace_config.apply()?;

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

        // Save PID before transferring ownership to SpawnedProcess::new_with_cgroup().
        // This allows us to kill the child if it fails, preventing zombie processes.
        let child_pid = child.id();

        // Create the SpawnedProcess wrapper.
        // If this fails, kill the child to prevent orphaned processes.
        let spawned_process: super::SpawnedProcess =
            match super::SpawnedProcess::new_with_cgroup(child, config.clone(), log_handles, cgroup) {
                Ok(sp) => sp,
                Err(e) => {
                    // Kill the child process using the saved PID
                    let pid = Pid::from_raw(child_pid as i32);
                    let _ = kill(pid, Signal::SIGKILL);
                    tracing::error!(
                        "Failed to create SpawnedProcess, killed child PID {}: {}",
                        child_pid,
                        e
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
    pub fn open_log_files(log_path: &str) -> Result<Option<(File, File)>> {
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

#[cfg(test)]
mod tests {
    use crate::supervisor::config::{WorkerConfig, WorkerInfo, WorkerOptions};
    use crate::supervisor::process::ProcessManager;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn minimal_config(command: &str, working_dir: &str, log_file: &str) -> WorkerConfig {
        WorkerConfig {
            worker: WorkerInfo {
                app: "testapp".to_string(),
                kind: "web".to_string(),
                command: command.to_string(),
                ordinal: 1,
            },
            env: HashMap::new(),
            options: WorkerOptions {
                working_dir: working_dir.to_string(),
                log_file: log_file.to_string(),
                uid: None,
                gid: None,
                timeout: 30,
                grace_period: 2,
                max_restarts: 3,
                health_check: None,
                isolation: None,
            },
        }
    }

    #[test]
    fn test_open_log_files_creates_file_and_dirs() {
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("subdir").join("app.log");
        let handles = ProcessManager::open_log_files(log_path.to_str().unwrap())
            .expect("open_log_files should succeed");
        assert!(handles.is_some(), "should return file handles");
        assert!(log_path.exists(), "log file should be created on disk");
    }

    #[test]
    fn test_spawn_process_echo_succeeds() {
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("test.log");

        let config = minimal_config(
            "echo hello",
            tmp.path().to_str().unwrap(),
            log_path.to_str().unwrap(),
        );

        let mut pm = ProcessManager::new().expect("ProcessManager::new should succeed");
        pm.spawn_process(&config)
            .expect("spawning 'echo hello' should succeed");

        // Allow log-capture threads to drain before asserting count.
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(
            pm.get_process_count(),
            1,
            "one process should be registered"
        );
    }

    #[test]
    fn test_spawn_duplicate_process_id_replaces_old() {
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("test.log");

        let config = minimal_config(
            "sleep 60",
            tmp.path().to_str().unwrap(),
            log_path.to_str().unwrap(),
        );

        let mut pm = ProcessManager::new().expect("ProcessManager::new should succeed");
        pm.spawn_process(&config)
            .expect("first spawn should succeed");
        assert_eq!(pm.get_process_count(), 1);

        // Spawning again with the same app/kind/ordinal replaces the old entry.
        pm.spawn_process(&config)
            .expect("second spawn should succeed");
        assert_eq!(
            pm.get_process_count(),
            1,
            "duplicate should replace, not add"
        );
    }
}
