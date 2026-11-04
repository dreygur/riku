//! Path and environment configuration for riku.
//!
//! Security model: every path riku reads or writes hangs off `$RIKU_ROOT`
//! (default `$HOME/.riku`). Those two variables are the only path inputs this
//! crate takes from outside, and they are accepted only as absolute paths with
//! no `..`, so the resolved tree never depends on the working directory or
//! climbs out of the directory the operator named.

use anyhow::Result;
use std::env;
use std::path::{Path, PathBuf};

pub mod container_engine;
pub mod nginx_host;
pub mod program;
pub mod reload_programs;
mod root_dirs;
pub mod router;
pub mod settings;

use root_dirs::root_dirs;

pub use nginx_host::{NginxHostPaths, DEFAULT_NGINX_INCLUDE_FILE, DEFAULT_NGINX_SITES_ENABLED};
pub use reload_programs::ReloadPrograms;

/// The one-line installer that fetches and checks the latest release binary.
pub const INSTALL_SCRIPT_URL: &str =
    "https://raw.githubusercontent.com/dreygur/riku/main/scripts/install.sh";

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
    pub riku_script: PathBuf,
    pub plugin_root: PathBuf,
    pub app_root: PathBuf,
    pub data_root: PathBuf,
    pub env_root: PathBuf,
    pub git_root: PathBuf,
    pub log_root: PathBuf,
    pub nginx_root: PathBuf,
    /// Per app Caddy site files, imported by `/etc/caddy/Caddyfile`.
    pub caddy_root: PathBuf,
    /// Holds the host router name. See [`router::Router::load`].
    pub router_file: PathBuf,
    /// Holds the container engine choice. See [`container_engine::load`].
    pub container_engine_file: PathBuf,
    /// Holds the server wide settings. See [`settings::load`].
    pub settings_file: PathBuf,
    pub cache_root: PathBuf,
    pub workers_root: PathBuf,
    pub workers_available: PathBuf,
    pub workers_enabled: PathBuf,
    pub acme_root: PathBuf,
    pub acme_www: PathBuf,
    /// Where older riku versions symlinked each app config
    /// (`/etc/nginx/sites-enabled`). riku no longer writes here: a root
    /// `riku init` removes the links that point into `nginx_root` and
    /// `riku doctor` reports any left. See [`NginxHostPaths`] for overrides.
    pub nginx_sites_enabled: PathBuf,
    /// The root-owned file that makes nginx load `nginx_root/*.conf`
    /// (`/etc/nginx/conf.d/riku.conf`). Only a root `riku init` writes it.
    /// See [`NginxHostPaths`] for overrides.
    pub nginx_include_file: PathBuf,
    /// Programs a router reload runs. Tests replace them with mock scripts.
    pub reload_programs: ReloadPrograms,
}

impl RikuPaths {
    /// Returns the deploy log file path for a given app: `{log_root}/{app}/deploy.log`.
    pub fn deploy_log_file(&self, app: &str) -> PathBuf {
        self.log_root.join(app).join("deploy.log")
    }

    /// The file `riku-nginx.path` watches. riku writes it once per nginx
    /// reload request, and nothing else touches it, so config writes in
    /// `nginx_root` never start a reload on their own.
    pub fn nginx_reload_trigger(&self) -> PathBuf {
        self.riku_root.join("nginx-reload.request")
    }

    /// Build paths using the given root directory and home directory, with
    /// the system nginx paths from the environment ([`NginxHostPaths::from_env`]).
    pub fn from_dirs(riku_root: PathBuf, home: &Path) -> Self {
        Self::with_nginx_host(riku_root, home, NginxHostPaths::from_env())
    }

    /// [`Self::from_dirs`] with the system nginx paths given.
    pub fn with_nginx_host(riku_root: PathBuf, home: &Path, nginx_host: NginxHostPaths) -> Self {
        let riku_script = env::current_exe().unwrap_or_else(|_| PathBuf::from("riku"));
        let acme_root = env::var("RIKU_ACME_ROOT")
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
            caddy_root: riku_root.join("caddy"),
            router_file: riku_root.join(router::ROUTER_FILE),
            container_engine_file: riku_root.join(container_engine::ENGINE_FILE),
            settings_file: riku_root.join(settings::SETTINGS_FILE),
            cache_root: riku_root.join("cache"),
            workers_root: riku_root.join("workers"),
            workers_available: riku_root.join("workers-available"),
            workers_enabled: riku_root.join("workers-enabled"),
            acme_www: riku_root.join("acme"),
            riku_root,
            riku_script,
            acme_root,
            nginx_sites_enabled: nginx_host.sites_enabled,
            nginx_include_file: nginx_host.include_file,
            reload_programs: ReloadPrograms::default(),
        }
    }

    /// Build paths from the environment, honoring `$RIKU_ROOT` and `$HOME`.
    ///
    /// Falls back to `$HOME/.riku` when `RIKU_ROOT` is not set. Returns an
    /// error instead of panicking when `$HOME` is unset, which happens under
    /// forced-command SSH/git-shell setups that strip the caller's
    /// environment before invoking riku. Also errors when either value is
    /// relative or contains `..`.
    pub fn from_env() -> Result<Self> {
        match root_dirs(env::var("HOME").ok(), env::var("RIKU_ROOT").ok()) {
            Ok((riku_root, home)) => Ok(Self::from_dirs(riku_root, &home)),
            Err(e) => Err(e),
        }
    }

    /// Build paths rooted at `root/.riku`, using `root` as home too, the
    /// `RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path())` shape that
    /// otherwise gets hand-copied into nearly every crate's test module.
    /// Not `#[cfg(test)]`: other crates' tests need to call it too, and that
    /// attribute doesn't cross crate boundaries.
    ///
    /// The system nginx paths are placed inside `root` whatever the
    /// environment says, so a test never touches the machine's real nginx.
    pub fn for_tests(root: &Path) -> Self {
        let nginx_host = NginxHostPaths {
            sites_enabled: root.join("nginx-sites-enabled"),
            include_file: root.join("nginx-conf.d").join("riku.conf"),
        };
        Self::with_nginx_host(root.join(".riku"), root, nginx_host)
    }
}

#[cfg(test)]
mod tests;
