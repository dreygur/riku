//! Where the system nginx keeps the two paths riku touches outside its own
//! root: the legacy `sites-enabled` directory and the include file.
//!
//! Security: root init writes the include file and deletes links in
//! sites-enabled, so it must not take these paths from an environment a
//! deploy user can influence (`sudo -E` keeps the caller's variables). Root
//! init uses [`NginxHostPaths::system`]; everything else, which only reads
//! these paths, honors the overrides.

use std::env;
use std::path::PathBuf;

/// Where nginx looks for enabled site configs on a stock Debian/Ubuntu install.
pub const DEFAULT_NGINX_SITES_ENABLED: &str = "/etc/nginx/sites-enabled";

/// The one file riku installs under `/etc/nginx`. The `conf.d` directory is
/// included inside the `http` block by the Debian, Ubuntu, RHEL and nginx.org
/// packages alike.
pub const DEFAULT_NGINX_INCLUDE_FILE: &str = "/etc/nginx/conf.d/riku.conf";

/// The system nginx paths, after any override.
#[derive(Debug, Clone, PartialEq)]
pub struct NginxHostPaths {
    /// `RIKU_NGINX_SITES_ENABLED`, else [`DEFAULT_NGINX_SITES_ENABLED`].
    pub sites_enabled: PathBuf,
    /// `RIKU_NGINX_HOST_INCLUDE`, else [`DEFAULT_NGINX_INCLUDE_FILE`].
    pub include_file: PathBuf,
}

impl NginxHostPaths {
    /// The paths with this process's overrides.
    pub fn from_env() -> Self {
        Self::resolve(
            env::var("RIKU_NGINX_SITES_ENABLED").ok(),
            env::var("RIKU_NGINX_HOST_INCLUDE").ok(),
        )
    }

    /// The distribution paths, ignoring every override. Root init uses it.
    pub fn system() -> Self {
        Self::resolve(None, None)
    }

    /// The paths for the given override values, without reading the
    /// environment.
    pub fn resolve(sites_enabled: Option<String>, host_include: Option<String>) -> Self {
        Self {
            sites_enabled: PathBuf::from(
                sites_enabled.unwrap_or_else(|| DEFAULT_NGINX_SITES_ENABLED.to_string()),
            ),
            include_file: PathBuf::from(
                host_include.unwrap_or_else(|| DEFAULT_NGINX_INCLUDE_FILE.to_string()),
            ),
        }
    }
}
