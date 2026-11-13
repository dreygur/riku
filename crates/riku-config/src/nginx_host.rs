//! Where the system nginx keeps the one path riku touches outside its own
//! root: the include file that loads riku's sites.
//!
//! Security: root init writes the include file, so it must not take its path
//! from an environment a deploy user can influence (`sudo -E` keeps the
//! caller's variables). Root init uses [`NginxHostPaths::system`]; everything
//! else, which only reads the path, honors the override.

use std::env;
use std::path::{Path, PathBuf};

/// The one file riku installs under `/etc/nginx`. The `conf.d` directory is
/// included inside the `http` block by the Debian, Ubuntu, RHEL and nginx.org
/// packages alike.
pub const DEFAULT_NGINX_INCLUDE_FILE: &str = "/etc/nginx/conf.d/riku.conf";

/// Alpine's nginx package includes `http.d` inside the `http` block and has
/// no `conf.d`.
pub const ALPINE_NGINX_INCLUDE_FILE: &str = "/etc/nginx/http.d/riku.conf";

/// The include file for this host: [`DEFAULT_NGINX_INCLUDE_FILE`], unless
/// its directory is missing and Alpine's `http.d` exists. `is_dir` is
/// [`Path::is_dir`] outside tests.
pub fn host_include_file(is_dir: impl Fn(&Path) -> bool) -> PathBuf {
    let default = Path::new(DEFAULT_NGINX_INCLUDE_FILE);
    let alpine = Path::new(ALPINE_NGINX_INCLUDE_FILE);
    let in_dir = |file: &Path| file.parent().is_some_and(&is_dir);
    if !in_dir(default) && in_dir(alpine) {
        alpine.to_path_buf()
    } else {
        default.to_path_buf()
    }
}

/// The system nginx paths, after any override.
#[derive(Debug, Clone, PartialEq)]
pub struct NginxHostPaths {
    /// `RIKU_NGINX_HOST_INCLUDE`, else [`host_include_file`].
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
    /// environment. With no override the host's directories pick the file.
    pub fn resolve(host_include: Option<String>) -> Self {
        Self {
            include_file: match host_include {
                Some(file) => PathBuf::from(file),
                None => host_include_file(Path::is_dir),
            },
        }
    }
}
