//! Process spawning logic for the ProcessManager.

use anyhow::Result;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use nix::unistd::{Gid, Uid};
use std::fs::{File, OpenOptions};
use std::process::{Command, Stdio};
use std::thread;

use crate::config::WorkerConfig;

use super::log_capture::run_log_capture_thread;
use super::ProcessManager;

impl ProcessManager {
    /// Spawn a new process based on the worker configuration.
    #[allow(clippy::question_mark)]
    pub fn spawn_process(&mut self, config: &WorkerConfig) -> Result<()> {
        use std::os::unix::process::CommandExt;

        let app_name = &config.worker.app;
        let worker_kind = &config.worker.kind;
        let ordinal = config.worker.ordinal;

        let process_id = config.worker.name().to_string();

        if let Some(sandbox) = &config.options.sandbox {
            if let Err(e) = sandbox.check_root() {
                return Err(e.context(format!("{process_id} refuses to start")));
            }
        }

        if self.processes.contains_key(&process_id) {
            tracing::info!("Process {} already exists, stopping it first", process_id);
            if let Err(e) = self.stop_process_by_id(&process_id) {
                return Err(e);
            }
        }

        let log_path = &config.options.log_file;
        let log_handles = match Self::open_log_files(log_path) {
            Ok(handles) => handles,
            Err(e) => return Err(e),
        };

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

        // Provision the worker's cgroup (when it sets a limit or a sandbox)
        // before spawning, so the constraints already exist when the
        // worker joins it from within pre_exec.
        let cgroup = match self.worker_cgroup(config) {
            Ok(cgroup) => cgroup,
            Err(e) => return Err(e),
        };
        let cgroup_for_child = cgroup.clone();

        // A sandboxed worker is started through the `riku __ns-shim`
        // subcommand instead of directly: it does the unshare/fork/exec
        // dance itself, from its own `main`, well after this
        // `Command::spawn()` has returned. See `isolation/mod.rs` for why
        // that fork can't happen inside this process's own `pre_exec`.
        let mut cmd = match &config.options.sandbox {
            Some(sandbox) => {
                let shim_exe = match std::env::current_exe() {
                    Ok(exe) => exe,
                    Err(e) => return Err(e.into()),
                };
                let mut c = Command::new(shim_exe);
                c.arg("__ns-shim").env("RIKU_NS_ROOT", &sandbox.root_dir);
                c.env("RIKU_NS_CMD", &config.worker.command);
                c
            }
            None => {
                let mut c = Command::new("sh");
                c.arg("-c").arg(&config.worker.command);
                c
            }
        };

        // Clone resource limits for use in pre_exec closure
        let limits = self.resource_limits.clone();

        // Set resource limits to prevent runaway processes (pre_exec is unsafe)
        unsafe {
            cmd.current_dir(&config.options.working_dir)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                // Create new process group for proper signal handling
                .process_group(0)
                // Set resource limits in child process before exec
                .pre_exec(move || {
                    // Drop to configured gid/uid if specified (gid must be set before uid).
                    if let Some(gid) = target_gid {
                        if let Err(e) = nix::unistd::setgid(gid) {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::PermissionDenied,
                                e.to_string(),
                            ));
                        }
                    }
                    if let Some(uid) = target_uid {
                        if let Err(e) = nix::unistd::setuid(uid) {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::PermissionDenied,
                                e.to_string(),
                            ));
                        }
                    }

                    // Join the cgroup using our own (real, top-level) PID.
                    // This is the same PID whether we're about to exec the
                    // worker directly or exec into `__ns-shim`: the cgroup
                    // membership is inherited across both the shim's own
                    // fork and its exec of the real worker.
                    if let Some(cgroup) = &cgroup_for_child {
                        if let Err(e) = cgroup.add_self() {
                            return Err(e);
                        }
                    }

                    limits.apply()
                });
        }

        // Set environment variables
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => return Err(e.into()),
        };

        // Take the pipes now (must happen before `child` is moved into
        // SpawnedProcess below), but don't start the reader threads yet,
        // they must only run once the wrapper construction below has
        // actually succeeded, otherwise a construction failure leaves
        // threads reading from a child we're about to SIGKILL.
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // Clone the log file handles for the reader threads. The reader
        // threads' cloned descriptors keep the log files open independently,
        // so the original `log_handles` can be dropped right after this.
        let log_handles_for_threads = match &log_handles {
            Some((stdout_log, stderr_log)) => {
                match (stdout_log.try_clone(), stderr_log.try_clone()) {
                    (Ok(stdout_log), Ok(stderr_log)) => Some((stdout_log, stderr_log)),
                    (Err(e), _) | (_, Err(e)) => return Err(e.into()),
                }
            }
            None => None,
        };

        // Save PID before transferring ownership to SpawnedProcess::new_with_cgroup().
        // This allows us to kill the child if it fails, preventing zombie processes.
        let child_pid = child.id();

        // Create the SpawnedProcess wrapper.
        // If this fails, kill the child to prevent orphaned processes.
        let spawned_process: super::SpawnedProcess = match super::SpawnedProcess::new_with_cgroup(
            child,
            config.clone(),
            cgroup,
            self.pid_dir.as_deref(),
        ) {
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

        // Now that the wrapper exists, start the log capture threads.
        if let Some((stdout_log, stderr_log)) = log_handles_for_threads {
            if let Some(stdout_reader) = stdout {
                let path = std::path::PathBuf::from(log_path);
                thread::spawn(move || {
                    run_log_capture_thread(stdout_reader, &path, stdout_log, "stdout");
                });
            }

            if let Some(stderr_reader) = stderr {
                let path = std::path::PathBuf::from(log_path);
                thread::spawn(move || {
                    run_log_capture_thread(stderr_reader, &path, stderr_log, "stderr");
                });
            }
        }

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
#[path = "spawn_tests.rs"]
mod tests;
