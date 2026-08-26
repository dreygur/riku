use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};

/// Raw source URL for fetching the latest riku script (for reference implementation).
pub const RIKU_RAW_SOURCE_URL: &str =
    "https://raw.githubusercontent.com/dreygur/riku/master/src/main.rs";

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

/// Where nginx looks for enabled site configs on a stock Debian/Ubuntu install.
pub const DEFAULT_NGINX_SITES_ENABLED: &str = "/etc/nginx/sites-enabled";

/// All resolved directory paths used by riku.
#[derive(Debug, Clone)]
pub struct RikuPaths {
    pub riku_root: PathBuf,
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
    pub acme_root: PathBuf,
    pub acme_www: PathBuf,
    /// Directory nginx reads enabled site configs from. Every generated app
    /// config is symlinked in here, so unlike the fields above it is a
    /// system-wide path (`/etc/nginx/sites-enabled`), not one under
    /// `riku_root`. `RIKU_NGINX_SITES_ENABLED` overrides it so a test run can
    /// point it somewhere disposable instead of writing to the real nginx.
    pub nginx_sites_enabled: PathBuf,
}

impl RikuPaths {
    /// Returns the deploy log file path for a given app: `{log_root}/{app}/deploy.log`.
    pub fn deploy_log_file(&self, app: &str) -> PathBuf {
        self.log_root.join(app).join("deploy.log")
    }

    /// Build paths using the given root directory and home directory.
    ///
    /// This is the core constructor used by both production code and tests.
    pub fn from_dirs(riku_root: PathBuf, home: &Path) -> Self {
        let riku_script = env::current_exe().unwrap_or_else(|_| PathBuf::from("riku"));

        let acme_root = env::var("ACME_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".acme.sh"));

        let nginx_sites_enabled = env::var("RIKU_NGINX_SITES_ENABLED")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_NGINX_SITES_ENABLED));

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
            riku_script,
            acme_root,
            nginx_sites_enabled,
        }
    }

    /// Build paths from the environment, honoring `$RIKU_ROOT` and `$HOME`.
    ///
    /// Falls back to `$HOME/.riku` when `RIKU_ROOT` is not set. Returns an
    /// error instead of panicking when `$HOME` is unset, which happens under
    /// forced-command SSH/git-shell setups that strip the caller's
    /// environment before invoking riku.
    pub fn from_env() -> Result<Self> {
        let home = match env::var("HOME") {
            Ok(value) => PathBuf::from(value),
            Err(e) => return Err(e).context("HOME environment variable must be set"),
        };
        let riku_root = env::var("RIKU_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".riku"));
        Ok(Self::from_dirs(riku_root, &home))
    }

    /// Build paths rooted at `root/.riku`, using `root` as home too, the
    /// `RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path())` shape that
    /// otherwise gets hand-copied into nearly every crate's test module.
    /// Not `#[cfg(test)]`: other crates' tests need to call it too, and that
    /// attribute doesn't cross crate boundaries.
    pub fn for_tests(root: &Path) -> Self {
        let mut paths = Self::from_dirs(root.join(".riku"), root);
        // Ignore any RIKU_NGINX_SITES_ENABLED the surrounding process set: a
        // test must never symlink into the machine's real nginx.
        paths.nginx_sites_enabled = root.join("nginx-sites-enabled");
        paths
    }
}

#[cfg(test)]
mod tests;
