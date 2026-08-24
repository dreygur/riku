//! Process spawning logic for the ProcessManager.

use anyhow::Result;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use nix::unistd::{Gid, Uid};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::cgroups::{CgroupLimits, WorkerCgroup};
use crate::config::WorkerConfig;

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
                    pids_max: opts.max_pids,
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

        // When namespace isolation is enabled, exec the `riku __ns-shim`
        // subcommand instead of the worker command directly: it does the
        // unshare/fork/exec dance itself, from its own `main`, well after
        // this `Command::spawn()` has returned. See isolation.rs for why
        // that fork can't happen inside this process's own `pre_exec`.
        let mut cmd = if namespace_config.enabled {
            let shim_exe = std::env::current_exe()?;
            let mut c = Command::new(shim_exe);
            c.arg("__ns-shim").env(
                "RIKU_NS_ROOT",
                namespace_config.isolated_root.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("isolation enabled but no root_dir configured")
                })?,
            );
            c.env("RIKU_NS_CMD", &config.worker.command);
            c
        } else {
            let mut c = Command::new("sh");
            c.arg("-c").arg(&config.worker.command);
            c
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
                        nix::unistd::setgid(gid).map_err(|e| {
                            std::io::Error::new(std::io::ErrorKind::PermissionDenied, e.to_string())
                        })?;
                    }
                    if let Some(uid) = target_uid {
                        nix::unistd::setuid(uid).map_err(|e| {
                            std::io::Error::new(std::io::ErrorKind::PermissionDenied, e.to_string())
                        })?;
                    }

                    // Join the cgroup using our own (real, top-level) PID.
                    // This is the same PID whether we're about to exec the
                    // worker directly or exec into `__ns-shim`: the cgroup
                    // membership is inherited across both the shim's own
                    // fork and its exec of the real worker.
                    if let Some(cgroup) = &cgroup_for_child {
                        cgroup.add_self()?;
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
                Some((stdout_log.try_clone()?, stderr_log.try_clone()?))
            }
            None => None,
        };

        // Save PID before transferring ownership to SpawnedProcess::new_with_cgroup().
        // This allows us to kill the child if it fails, preventing zombie processes.
        let child_pid = child.id();

        // Create the SpawnedProcess wrapper.
        // If this fails, kill the child to prevent orphaned processes.
        let spawned_process: super::SpawnedProcess =
            match super::SpawnedProcess::new_with_cgroup(child, config.clone(), cgroup) {
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

/// How often (at most) to `stat()` the log path to check for external
/// rotation, throttled by wall-clock time rather than line count so the
/// check cost stays constant regardless of how chatty the worker's stdout
/// is.
const ROTATION_CHECK_INTERVAL: Duration = Duration::from_secs(2);

/// Read `reader` line by line, appending each line to `file` (opened on
/// `log_path`), staying correct across two different kinds of external log
/// rotation:
///
/// - **copytruncate** (same inode, file truncated to 0 in place): already
///   handled for free: `file` was opened with `O_APPEND`, so the kernel
///   seeks to the file's *current* end before every `write()`, which after
///   a truncate is simply offset 0. No detection needed, no hole, no panic.
/// - **rename + recreate** (the conventional `logrotate` default): the path
///   now refers to a *different* inode than the one `file` has open, `file`
///   would otherwise keep appending into the renamed-away copy, invisible
///   to anything tailing the original path. Detected here by periodically
///   comparing `fstat(file)` against `stat(log_path)`; on a mismatch, the
///   file is reopened at `log_path` (in append mode) and capture continues
///   through the new handle, with nothing more than a log line lost in the
///   window between rotation and the next check.
///
/// Never panics: every fallible step (write, flush, reopen, the rotation
/// check's own stat calls) degrades to "log this line was dropped" rather
/// than crashing the thread, since losing a log line is recoverable and
/// killing the capture thread silently is not.
/// If `log_path` now refers to a different inode than `file` has open,
/// reopen it in append mode and swap `*file` for the new handle. A no-op
/// when nothing has rotated, when the path is temporarily missing
/// (deleted but not yet recreated: kept writing through the existing,
/// still-valid-just-unlinked fd until the next check), or when reopening
/// itself fails (logged, old handle kept so lines keep landing somewhere
/// rather than being dropped entirely).
fn reopen_if_rotated(log_path: &std::path::Path, file: &mut File, stream_name: &str) {
    use std::os::unix::fs::MetadataExt;

    let fd_inode = file.metadata().ok().map(|m| (m.dev(), m.ino()));
    let path_inode = match fs::metadata(log_path) {
        Ok(m) => (m.dev(), m.ino()),
        Err(_) => return,
    };

    if fd_inode == Some(path_inode) {
        return;
    }

    let _ = file.flush();
    match OpenOptions::new().create(true).append(true).open(log_path) {
        Ok(reopened) => {
            tracing::info!(
                "{} log file rotated externally, reopened: {}",
                stream_name,
                log_path.display()
            );
            *file = reopened;
        }
        Err(e) => {
            tracing::warn!(
                "Failed to reopen rotated {} log {}: {}, continuing to write to the old file handle",
                stream_name,
                log_path.display(),
                e
            );
        }
    }
}

/// Read `reader` line by line, appending each line to `file` (opened on
/// `log_path`), staying correct across two different kinds of external log
/// rotation:
///
/// - **copytruncate** (same inode, file truncated to 0 in place): already
///   handled for free: `file` was opened with `O_APPEND`, so the kernel
///   seeks to the file's *current* end before every `write()`, which after
///   a truncate is simply offset 0. No detection needed, no hole, no panic.
/// - **rename + recreate** (the conventional `logrotate` default): the path
///   now refers to a *different* inode than the one `file` has open,
///   detected by [`reopen_if_rotated`], called at most once per
///   [`ROTATION_CHECK_INTERVAL`] so the check cost stays constant
///   regardless of log volume.
///
/// Never panics: every fallible step (write, flush, reopen) degrades to
/// "this line was dropped" rather than crashing the thread, since losing a
/// log line is recoverable and killing the capture thread silently is not.
fn run_log_capture_thread(
    reader: impl std::io::Read,
    log_path: &std::path::Path,
    mut file: File,
    stream_name: &'static str,
) {
    let mut last_rotation_check = Instant::now();
    let reader = BufReader::new(reader);

    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                tracing::debug!("Error reading {}: {}", stream_name, e);
                break;
            }
        };

        if last_rotation_check.elapsed() >= ROTATION_CHECK_INTERVAL {
            last_rotation_check = Instant::now();
            reopen_if_rotated(log_path, &mut file, stream_name);
        }

        let _ = writeln!(file, "{}", line);
        let _ = file.flush();
    }

    drop(file);
    tracing::debug!("{} log capture thread exited", stream_name);
}

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod tests;
