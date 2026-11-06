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

/// Default worker timeout in seconds (2 hours).
pub const RIKU_WORKER_TIMEOUT: u64 = 7200;

/// Default worker grace period for shutdown in seconds.
pub const RIKU_WORKER_GRACE_PERIOD: u64 = 30;

/// Default max restart attempts before marking app as failed.
pub const RIKU_MAX_RESTARTS: u32 = 5;

/// Where uWSGI copies put their unix sockets on a real host. It sits outside
/// the deploy user's home so nginx needs no access to that home. Root init
/// creates it owned by the deploy user, group [`SOCKET_GROUP`], mode `2750`.
pub const SOCKET_ROOT: &str = "/run/riku-sockets";

/// The group shared by the deploy user and the nginx worker user, and the
/// only group that can reach [`SOCKET_ROOT`].
pub const SOCKET_GROUP: &str = "riku-sockets";

/// The router helper's socket. The helper binds it itself, directly in `/run`
/// so the deploy user needs no access to any directory below it.
pub const ROUTER_SOCKET: &str = "/run/riku-router.sock";

/// Exists on a host that booted with systemd as init.
pub const SYSTEMD_RUNTIME_DIR: &str = "/run/systemd/system";

/// Where Caddy site files live on a real host. It sits outside the deploy
/// user's home so the `caddy` user needs no access to that home. Root init
/// creates it owned by root, group `caddy`, mode `2750`; only the router
/// helper writes site files here.
pub const CADDY_SITE_ROOT: &str = "/etc/riku/caddy";

/// Root-owned `0755` directory of the nginx site configs the router helper
/// renders, one `<app>.conf` each. The host include loads `*.conf` from here.
pub const NGINX_SITE_ROOT: &str = "/etc/riku/nginx";

/// Root-owned `0700` directory of per app TLS material, `<app>/fullchain.pem`
/// and `<app>/key.pem`, written by the router helper.
pub const CERT_ROOT: &str = "/etc/riku/certs";

/// Root-owned `0755` parent of the directories nginx serves files from.
pub const VAR_ROOT: &str = "/var/lib/riku";

/// The ACME http-01 webroot on a real host. Root init creates it owned by
/// the deploy user, group [`SOCKET_GROUP`], mode `2750`, so the nginx worker
/// reads challenge files without any access to the deploy home.
pub const ACME_WEBROOT: &str = "/var/lib/riku/acme";

/// Where deploys publish each app's static files for nginx, one directory
/// per app. Same owner, group and mode as [`ACME_WEBROOT`].
pub const STATIC_ROOT: &str = "/var/lib/riku/static";

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
    /// The uWSGI socket directory, [`SOCKET_ROOT`] outside tests. Not taken
    /// from the environment: root init creates it and must not be steered.
    pub socket_root: PathBuf,
    /// Per app Caddy site files, imported by `/etc/caddy/Caddyfile`.
    /// [`CADDY_SITE_ROOT`] outside tests, not taken from the environment for
    /// the same reason as `socket_root`.
    pub caddy_root: PathBuf,
    /// The router helper's socket, [`ROUTER_SOCKET`] outside tests.
    pub router_socket: PathBuf,
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
    /// The acme.sh install, `$RIKU_ACME_ROOT` or `~/.acme.sh`.
    pub acme_sh_root: PathBuf,
    /// Per app links to the acme.sh domain directory holding the app's
    /// certificate, so destroy can find it. Inside the deploy home.
    pub acme_cert_links: PathBuf,
    /// The ACME http-01 webroot nginx serves challenges from,
    /// [`ACME_WEBROOT`] outside tests, not taken from the environment for the
    /// same reason as `socket_root`.
    pub acme_root: PathBuf,
    /// Published static files, `<static_root>/<app>`. [`STATIC_ROOT`]
    /// outside tests, not taken from the environment either.
    pub static_root: PathBuf,
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

    /// Build paths using the given root directory and home directory, with
    /// the system nginx paths from the environment ([`NginxHostPaths::from_env`]).
    pub fn from_dirs(riku_root: PathBuf, home: &Path) -> Self {
        Self::with_nginx_host(riku_root, home, NginxHostPaths::from_env())
    }

    /// [`Self::from_dirs`] with the system nginx paths given.
    pub fn with_nginx_host(riku_root: PathBuf, home: &Path, nginx_host: NginxHostPaths) -> Self {
        let riku_script = env::current_exe().unwrap_or_else(|_| PathBuf::from("riku"));
        let acme_sh_root = env::var("RIKU_ACME_ROOT")
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
            socket_root: PathBuf::from(SOCKET_ROOT),
            caddy_root: PathBuf::from(CADDY_SITE_ROOT),
            router_socket: PathBuf::from(ROUTER_SOCKET),
            router_file: riku_root.join(router::ROUTER_FILE),
            container_engine_file: riku_root.join(container_engine::ENGINE_FILE),
            settings_file: riku_root.join(settings::SETTINGS_FILE),
            cache_root: riku_root.join("cache"),
            workers_root: riku_root.join("workers"),
            workers_available: riku_root.join("workers-available"),
            workers_enabled: riku_root.join("workers-enabled"),
            acme_cert_links: riku_root.join("acme"),
            acme_root: PathBuf::from(ACME_WEBROOT),
            static_root: PathBuf::from(STATIC_ROOT),
            riku_root,
            riku_script,
            acme_sh_root,
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
    /// environment says, so a test never touches the machine's real nginx,
    /// and so are the socket directory, `root/sockets`, the Caddy site
    /// directory, `root/caddy-sites`, the ACME webroot, `root/acme-webroot`,
    /// and the static root, `root/static`. Tests create the ones they use.
    pub fn for_tests(root: &Path) -> Self {
        let nginx_host = NginxHostPaths {
            sites_enabled: root.join("nginx-sites-enabled"),
            include_file: root.join("nginx-conf.d").join("riku.conf"),
        };
        let mut paths = Self::with_nginx_host(root.join(".riku"), root, nginx_host);
        paths.socket_root = root.join("sockets");
        paths.caddy_root = root.join("caddy-sites");
        paths.acme_root = root.join("acme-webroot");
        paths.static_root = root.join("static");
        paths
    }
}

#[cfg(test)]
mod tests;
