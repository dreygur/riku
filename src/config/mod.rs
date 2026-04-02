use std::env;
use std::path::{Path, PathBuf};

/// Raw source URL for fetching the latest riku script (for reference implementation).
pub const RIKU_RAW_SOURCE_URL: &str =
    "https://raw.githubusercontent.com/dreygur/riku/master/src/main.rs";

/// Default maximum log size for worker log files (in bytes).
#[allow(dead_code)]
pub const RIKU_LOG_MAXSIZE: u64 = 1048576;

/// Default worker timeout in seconds (2 hours).
pub const RIKU_WORKER_TIMEOUT: u64 = 7200;

/// Default worker grace period for shutdown in seconds.
pub const RIKU_WORKER_GRACE_PERIOD: u64 = 30;

/// Default max restart attempts before marking app as failed.
pub const RIKU_MAX_RESTARTS: u32 = 5;

/// Default nginx cache size in GB.
pub const NGINX_CACHE_SIZE_DEFAULT: u32 = 1;

/// Default nginx cache time in seconds (1 hour).
pub const NGINX_CACHE_TIME_DEFAULT: u32 = 3600;

/// Default nginx cache expiry in seconds (24 hours).
pub const NGINX_CACHE_EXPIRY_DEFAULT: u32 = 86400;

/// Default nginx cache redirects time in seconds (1 hour).
pub const NGINX_CACHE_REDIRECTS_DEFAULT: u32 = 3600;

/// Default nginx cache any time in seconds (1 hour).
pub const NGINX_CACHE_ANY_DEFAULT: u32 = 3600;

/// Default nginx cache control time in seconds (1 hour).
pub const NGINX_CACHE_CONTROL_DEFAULT: u32 = 3600;

/// All resolved directory paths used by riku.
#[derive(Debug, Clone)]
pub struct RikuPaths {
    pub riku_root: PathBuf,
    #[allow(dead_code)]
    pub riku_bin: PathBuf,
    pub riku_script: PathBuf,
    pub plugin_root: PathBuf,
    pub app_root: PathBuf,
    pub data_root: PathBuf,
    pub env_root: PathBuf,
    pub git_root: PathBuf,
    pub log_root: PathBuf,
    pub nginx_root: PathBuf,
    pub cache_root: PathBuf,
    pub workers_root: PathBuf,
    pub workers_available: PathBuf,
    pub workers_enabled: PathBuf,
    #[allow(dead_code)]
    pub acme_root: PathBuf,
    pub acme_www: PathBuf,
}

impl RikuPaths {
    /// Build paths using the given root directory and home directory.
    ///
    /// This is the core constructor used by both production code and tests.
    pub fn from_dirs(riku_root: PathBuf, home: &Path) -> Self {
        let riku_bin = home.join("bin");
        let riku_script = env::current_exe().unwrap_or_else(|_| PathBuf::from("riku"));

        let acme_root = env::var("ACME_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".acme.sh"));

        Self {
            plugin_root: riku_root.join("plugins"),
            app_root: riku_root.join("apps"),
            data_root: riku_root.join("data"),
            env_root: riku_root.join("envs"),
            git_root: riku_root.join("repos"),
            log_root: riku_root.join("logs"),
            nginx_root: riku_root.join("nginx"),
            cache_root: riku_root.join("cache"),
            workers_root: riku_root.join("workers"),
            workers_available: riku_root.join("workers-available"),
            workers_enabled: riku_root.join("workers-enabled"),
            acme_www: riku_root.join("acme"),
            riku_root,
            riku_bin,
            riku_script,
            acme_root,
        }
    }

    /// Build paths from the environment, honoring `$RIKU_ROOT` and `$HOME`.
    ///
    /// Falls back to `$HOME/.riku` when `RIKU_ROOT` is not set.
    pub fn from_env() -> Self {
        let home = PathBuf::from(env::var("HOME").expect("HOME environment variable must be set"));
        let riku_root = env::var("RIKU_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".riku"));
        Self::from_dirs(riku_root, &home)
    }
}

#[cfg(test)]
mod tests;
