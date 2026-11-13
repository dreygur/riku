//! Where the system nginx keeps the one path riku touches outside its own
//! root: the include file that loads riku's sites.
//!
//! Security: root init writes the include file, so it must not take its path
//! from an environment a deploy user can influence (`sudo -E` keeps the
//! caller's variables). Root init uses [`NginxHostPaths::system`]; everything
//! else, which only reads the path, honors the override.

use std::env;
use std::path::PathBuf;

/// The one file riku installs under `/etc/nginx`. The `conf.d` directory is
/// included inside the `http` block by the Debian, Ubuntu, RHEL and nginx.org
/// packages alike.
pub const DEFAULT_NGINX_INCLUDE_FILE: &str = "/etc/nginx/conf.d/riku.conf";

/// The system nginx paths, after any override.
#[derive(Debug, Clone, PartialEq)]
pub struct NginxHostPaths {
    /// `RIKU_NGINX_HOST_INCLUDE`, else [`DEFAULT_NGINX_INCLUDE_FILE`].
    pub include_file: PathBuf,
}

impl NginxHostPaths {
    /// The paths with this process's override.
    pub fn from_env() -> Self {
        Self::resolve(env::var("RIKU_NGINX_HOST_INCLUDE").ok())
    }

    /// The distribution paths, ignoring every override. Root init uses it.
    pub fn system() -> Self {
        Self::resolve(None)
    }

    /// The paths for the given override value, without reading the
    /// environment.
    pub fn resolve(host_include: Option<String>) -> Self {
        Self {
            include_file: PathBuf::from(
                host_include.unwrap_or_else(|| DEFAULT_NGINX_INCLUDE_FILE.to_string()),
            ),
        }
    }
}
