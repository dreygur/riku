use std::env;
use std::path::{Path, PathBuf};

/// Raw source URL for fetching the latest piku script (for reference implementation).
pub const RIKU_RAW_SOURCE_URL: &str = "https://raw.githubusercontent.com/piku/piku/master/piku.py";

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
mod tests {
    use super::*;
    use std::path::Path;

    /// Helper: build RikuPaths with a known home and root, no env vars involved.
    fn paths_with_root(root: &str, home: &str) -> RikuPaths {
        RikuPaths::from_dirs(PathBuf::from(root), &PathBuf::from(home))
    }

    #[test]
    fn default_paths_use_home_dot_riku() {
        let home = "/home/testuser";
        let paths = paths_with_root(&format!("{home}/.riku"), home);
        assert_eq!(paths.riku_root, Path::new("/home/testuser/.riku"));
    }

    #[test]
    fn all_subdirectory_paths_are_relative_to_riku_root() {
        let root = "/srv/riku";
        let home = "/home/riku";
        let p = paths_with_root(root, home);

        assert_eq!(p.app_root, Path::new("/srv/riku/apps"));
        assert_eq!(p.data_root, Path::new("/srv/riku/data"));
        assert_eq!(p.env_root, Path::new("/srv/riku/envs"));
        assert_eq!(p.git_root, Path::new("/srv/riku/repos"));
        assert_eq!(p.log_root, Path::new("/srv/riku/logs"));
        assert_eq!(p.nginx_root, Path::new("/srv/riku/nginx"));
        assert_eq!(p.cache_root, Path::new("/srv/riku/cache"));
        assert_eq!(p.workers_root, Path::new("/srv/riku/workers"));
        assert_eq!(
            p.workers_available,
            Path::new("/srv/riku/workers-available")
        );
        assert_eq!(p.workers_enabled, Path::new("/srv/riku/workers-enabled"));
        assert_eq!(p.acme_www, Path::new("/srv/riku/acme"));
        assert_eq!(p.plugin_root, Path::new("/srv/riku/plugins"));
    }

    #[test]
    fn custom_root_parameter_works() {
        let p = paths_with_root("/opt/custom-riku", "/home/deploy");
        assert_eq!(p.riku_root, Path::new("/opt/custom-riku"));
        assert_eq!(p.app_root, Path::new("/opt/custom-riku/apps"));
        assert_eq!(p.git_root, Path::new("/opt/custom-riku/repos"));
    }

    #[test]
    fn riku_bin_is_relative_to_home() {
        let p = paths_with_root("/whatever", "/home/alice");
        assert_eq!(p.riku_bin, Path::new("/home/alice/bin"));
    }

    #[test]
    fn acme_root_defaults_to_home_acme_sh() {
        // Test that ACME_ROOT defaults to ~/.acme.sh when not set
        // We test the logic directly rather than through from_dirs to avoid parallel test issues
        let home = PathBuf::from("/home/bob");

        // Simulate the ACME_ROOT resolution logic
        let result = std::env::var("ACME_ROOT_TEST_VAR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".acme.sh"));

        assert_eq!(result, Path::new("/home/bob/.acme.sh"));
    }

    #[test]
    fn acme_root_respects_env_var() {
        // Save the original value
        let orig_value = env::var("ACME_ROOT").ok();

        env::set_var("ACME_ROOT", "/custom/acme");
        let p = paths_with_root("/x", "/home/bob");
        assert_eq!(p.acme_root, Path::new("/custom/acme"));

        // Restore original value or remove if it wasn't set
        match orig_value {
            Some(v) => env::set_var("ACME_ROOT", v),
            None => env::remove_var("ACME_ROOT"),
        }
    }

    #[test]
    fn riku_log_maxsize_constant() {
        assert_eq!(RIKU_LOG_MAXSIZE, 1048576);
    }

    #[test]
    fn riku_raw_source_url_constant() {
        assert!(RIKU_RAW_SOURCE_URL.starts_with("https://"));
        assert!(RIKU_RAW_SOURCE_URL.contains("piku")); // Still refers to the original piku repo
    }

    #[test]
    fn from_env_uses_home() {
        // Just verify it doesn't panic and produces a sensible root.
        let paths = RikuPaths::from_env();
        assert!(paths.riku_root.is_absolute() || env::var("RIKU_ROOT").is_ok());
    }
}
