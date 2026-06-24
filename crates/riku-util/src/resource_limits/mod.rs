//! Resource limit configuration for spawned processes.
//!
//! Provides configurable resource limits (ulimit) to prevent runaway processes
//! and enable safe multi-tenant deployments.

use nix::sys::resource::{setrlimit, Resource};
use std::env;

#[cfg(test)]
mod tests;

/// Resource limits configuration for spawned processes
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum address space (virtual memory) in bytes (RLIMIT_AS)
    /// Default: 512 MB per process
    pub max_memory_bytes: Option<u64>,

    /// Maximum CPU time in seconds (RLIMIT_CPU)
    /// Default: 3600 seconds (1 hour)
    pub max_cpu_seconds: Option<u64>,

    /// Maximum number of open file descriptors (RLIMIT_NOFILE)
    /// Default: 1024
    pub max_open_files: Option<u64>,

    /// Maximum number of processes (RLIMIT_NPROC).
    ///
    /// Disabled (`None`) by default. `RLIMIT_NPROC` counts every process
    /// owned by the *real UID*, system-wide — not just this worker's
    /// process tree. Setting it here would falsely crash-loop a perfectly
    /// healthy worker the moment anything else running as the same UID
    /// (other workers, a shell, a desktop session) pushes that UID's total
    /// process count past the limit, and it provides no real per-worker
    /// isolation even when riku runs as its own dedicated user, since the
    /// budget is shared across every worker of every app under that UID.
    /// Operators who still want a UID-wide ceiling can opt back in via
    /// `RIKU_MAX_PROCESSES`. For real per-worker process-count isolation,
    /// use `IsolationOptions::max_pids` (cgroup v2 `pids.max`), which is
    /// scoped to exactly that worker's cgroup subtree.
    pub max_processes: Option<u64>,

    /// Maximum size of core files in bytes (RLIMIT_CORE)
    /// Default: 0 (disabled for security)
    pub max_core_file_bytes: Option<u64>,

    /// Maximum file size in bytes (RLIMIT_FSIZE)
    /// Default: 1 GB
    pub max_file_size_bytes: Option<u64>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: Some(512 * 1024 * 1024),     // 512 MB
            max_cpu_seconds: Some(3600),                   // 1 hour
            max_open_files: Some(1024),                    // 1024 files
            max_processes: None, // see field doc: UID-wide, not per-worker
            max_core_file_bytes: Some(0), // No core dumps
            max_file_size_bytes: Some(1024 * 1024 * 1024), // 1 GB
        }
    }
}

impl ResourceLimits {
    /// Load resource limits from environment variables.
    ///
    /// Environment variables:
    /// - RIKU_MAX_MEMORY_MB: RLIMIT_AS ceiling in MB (default: unset — see below)
    /// - RIKU_MAX_CPU_SECONDS: Maximum CPU time in seconds (default: 3600)
    /// - RIKU_MAX_OPEN_FILES: Maximum open files (default: 1024)
    /// - RIKU_MAX_PROCESSES: Maximum processes, RLIMIT_NPROC, UID-wide (default: disabled — see field doc)
    /// - RIKU_MAX_FILE_SIZE_MB: Maximum file size in MB (default: 1024)
    /// - RIKU_ENABLE_CORE_DUMPS: Enable core dumps (default: false)
    ///
    /// **Memory is opt-in.** `RLIMIT_AS` caps *virtual* address space, but
    /// language runtimes (node/v8, the JVM) reserve multiple GB of virtual
    /// memory at startup and abort under a tight cap — so applying a default
    /// `RLIMIT_AS` makes node/JVM builds and workers unrunnable. Resident
    /// memory for isolated workers is bounded by the cgroup `memory.max`
    /// instead (see `cgroups`). `RLIMIT_AS` is therefore enforced only when the
    /// operator explicitly sets `RIKU_MAX_MEMORY_MB`.
    pub fn from_env() -> Self {
        // Memory (RLIMIT_AS) is opt-in: no virtual-address cap unless the
        // operator asks for one. See the doc comment above for why.
        let mut limits = Self {
            max_memory_bytes: None,
            ..Self::default()
        };

        if let Ok(val) = env::var("RIKU_MAX_MEMORY_MB") {
            if let Ok(mb) = val.parse::<u64>() {
                limits.max_memory_bytes = Some(mb * 1024 * 1024);
                tracing::info!("Resource limit: max_memory = {} MB", mb);
            }
        }

        // CPU time limit in seconds
        if let Ok(val) = env::var("RIKU_MAX_CPU_SECONDS") {
            if let Ok(seconds) = val.parse::<u64>() {
                limits.max_cpu_seconds = Some(seconds);
                tracing::info!("Resource limit: max_cpu_time = {} seconds", seconds);
            }
        }

        // Open files limit
        if let Ok(val) = env::var("RIKU_MAX_OPEN_FILES") {
            if let Ok(count) = val.parse::<u64>() {
                limits.max_open_files = Some(count);
                tracing::info!("Resource limit: max_open_files = {}", count);
            }
        }

        // Max processes limit
        if let Ok(val) = env::var("RIKU_MAX_PROCESSES") {
            if let Ok(count) = val.parse::<u64>() {
                limits.max_processes = Some(count);
                tracing::info!("Resource limit: max_processes = {}", count);
            }
        }

        // File size limit in MB
        if let Ok(val) = env::var("RIKU_MAX_FILE_SIZE_MB") {
            if let Ok(mb) = val.parse::<u64>() {
                limits.max_file_size_bytes = Some(mb * 1024 * 1024);
                tracing::info!("Resource limit: max_file_size = {} MB", mb);
            }
        }

        // Core dumps (disabled by default for security)
        if env::var("RIKU_ENABLE_CORE_DUMPS").is_ok() {
            limits.max_core_file_bytes = None; // Unlimited
            tracing::warn!("Core dumps enabled - not recommended for production");
        }

        limits
    }

    /// Apply resource limits to the current process.
    ///
    /// This should be called in the child process after fork but before exec.
    /// Apply resource limits via setrlimit().
    ///
    /// # Safety
    /// This function is async-signal-safe and can be called from pre_exec().
    /// It ONLY uses async-signal-safe operations (setrlimit syscalls).
    ///
    /// ## CRITICAL: DO NOT add any code that:
    /// - Allocates memory (println!, eprintln!, format!, String::new, Vec::new, etc.)
    /// - Performs I/O (file operations, network, etc.)
    /// - Takes locks (Mutex, RwLock, etc.)
    /// - Calls non-async-signal-safe libc functions
    ///
    /// Violations will cause undefined behavior including:
    /// - Deadlocks (if signal interrupts a malloc/free)
    /// - Crashes (if heap is corrupted)
    /// - Silent data corruption
    ///
    /// This function runs AFTER fork() but BEFORE exec() in the child process.
    /// In this window, only async-signal-safe functions are allowed.
    ///
    /// See: https://man7.org/linux/man-pages/man7/signal-safety.7.html
    pub fn apply(&self) -> std::io::Result<()> {
        // Memory limit (address space)
        if let Some(bytes) = self.max_memory_bytes {
            setrlimit(Resource::RLIMIT_AS, bytes, bytes)
                .map_err(|e| std::io::Error::other(format!("Failed to set memory limit: {}", e)))?;
        }

        // CPU time limit
        if let Some(seconds) = self.max_cpu_seconds {
            setrlimit(Resource::RLIMIT_CPU, seconds, seconds)
                .map_err(|e| std::io::Error::other(format!("Failed to set CPU limit: {}", e)))?;
        }

        // Open files limit
        if let Some(count) = self.max_open_files {
            setrlimit(Resource::RLIMIT_NOFILE, count, count).map_err(|e| {
                std::io::Error::other(format!("Failed to set open files limit: {}", e))
            })?;
        }

        // Max processes limit
        if let Some(count) = self.max_processes {
            setrlimit(Resource::RLIMIT_NPROC, count, count).map_err(|e| {
                std::io::Error::other(format!("Failed to set process limit: {}", e))
            })?;
        }

        // Core file limit
        if let Some(bytes) = self.max_core_file_bytes {
            setrlimit(Resource::RLIMIT_CORE, bytes, bytes).map_err(|e| {
                std::io::Error::other(format!("Failed to set core file limit: {}", e))
            })?;
        }

        // File size limit
        if let Some(bytes) = self.max_file_size_bytes {
            setrlimit(Resource::RLIMIT_FSIZE, bytes, bytes).map_err(|e| {
                std::io::Error::other(format!("Failed to set file size limit: {}", e))
            })?;
        }

        Ok(())
    }

    /// Get a summary of configured limits for logging.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(mb) = self.max_memory_bytes.map(|b| b / 1024 / 1024) {
            parts.push(format!("mem={}MB", mb));
        }
        if let Some(s) = self.max_cpu_seconds {
            parts.push(format!("cpu={}s", s));
        }
        if let Some(n) = self.max_open_files {
            parts.push(format!("files={}", n));
        }
        if let Some(n) = self.max_processes {
            parts.push(format!("procs={}", n));
        }

        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(", ")
        }
    }
}
