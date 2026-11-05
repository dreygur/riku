//! The directories and programs the nginx side of the helper works with.
//!
//! Every path the helper writes or names in a config is built from one of
//! these roots and a checked [`AppName`]; no request field reaches a path.
//! Production values are the riku-config constants. Tests build their own
//! roots inside a temp directory, so no test touches `/etc` or changes the
//! process environment.

use std::path::{Path, PathBuf};

use riku_config::{
    ReloadPrograms, ACME_WEBROOT, CERT_ROOT, CLOUDFLARE_CONF, NGINX_CACHE_ROOT, NGINX_SITE_ROOT,
    SOCKET_ROOT, STATIC_ROOT,
};
use riku_container::types::AppName;
use riku_util::write_dir::ROOT_UID;

const SYSTEMD_RUNTIME_DIR: &str = "/run/systemd/system";

/// How the helper asks nginx to load a new config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadMethod {
    /// `systemctl reload nginx`, on a host that booted with systemd.
    Systemctl(PathBuf),
    /// `nginx -s reload`, everywhere else.
    Signal,
}

/// Where the helper keeps nginx's files and which programs it runs.
#[derive(Debug, Clone)]
pub struct NginxRoots {
    pub site_root: PathBuf,
    pub cert_root: PathBuf,
    pub cache_root: PathBuf,
    pub socket_root: PathBuf,
    pub static_root: PathBuf,
    pub acme_root: PathBuf,
    pub cloudflare_conf: PathBuf,
    pub nginx: PathBuf,
    pub reload: ReloadMethod,
    /// The uid every directory the helper writes into must belong to: root in
    /// production, the test's own uid in tests.
    pub owner_uid: u32,
    /// GiB all proxy caches together may be allowed, from `settings.toml`.
    pub cache_total_gb: u32,
}

impl NginxRoots {
    /// The roots of a real host, with programs looked up on `PATH`.
    /// `cache_total_gb` is the operator's `router.cache_total_gb`.
    pub fn production(cache_total_gb: u32) -> Self {
        let programs = ReloadPrograms::default();
        let reload = if Path::new(SYSTEMD_RUNTIME_DIR).is_dir() {
            ReloadMethod::Systemctl(programs.systemctl)
        } else {
            ReloadMethod::Signal
        };
        NginxRoots {
            site_root: PathBuf::from(NGINX_SITE_ROOT),
            cert_root: PathBuf::from(CERT_ROOT),
            cache_root: PathBuf::from(NGINX_CACHE_ROOT),
            socket_root: PathBuf::from(SOCKET_ROOT),
            static_root: PathBuf::from(STATIC_ROOT),
            acme_root: PathBuf::from(ACME_WEBROOT),
            cloudflare_conf: PathBuf::from(CLOUDFLARE_CONF),
            nginx: programs.nginx,
            reload,
            owner_uid: ROOT_UID,
            cache_total_gb,
        }
    }

    pub(crate) fn site_file(&self, app: &AppName) -> PathBuf {
        self.site_root.join(format!("{app}.conf"))
    }

    pub(crate) fn cert_dir(&self, app: &AppName) -> PathBuf {
        self.cert_root.join(app.as_str())
    }

    pub(crate) fn cache_dir(&self, app: &AppName) -> PathBuf {
        self.cache_root.join(app.as_str())
    }
}
