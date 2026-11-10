//! The own certificate files of a Caddy app.
//!
//! The pair is written under `<site root>/certs/<app>/` from the gate's
//! re-encoded PEM, never from a path in the request. Caddy reads them as the
//! `caddy` user, so both files are `0640` and the two directories `2750`:
//! the setgid bit makes each file take the group of the site root, which
//! root init sets to `caddy`, and no other account can read the key.

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use riku_caddy::TlsFiles;
use riku_container::types::AppName;

use crate::caddy_apply::{CaddyRoots, SITE_FILE};
use crate::nginx_render::{CERTIFICATE_FILE, KEY_FILE};
use crate::schema::Tls;
use crate::site_files::{ensure_dir, Planned};

const CERT_DIR: &str = "certs";
const CERT_DIR_MODE: u32 = 0o2750;

impl CaddyRoots {
    pub(crate) fn cert_root(&self) -> PathBuf {
        self.site_root.join(CERT_DIR)
    }

    pub(crate) fn cert_dir(&self, app: &AppName) -> PathBuf {
        self.cert_root().join(app.as_str())
    }

    /// The paths the site file names for `app`'s pair.
    pub(crate) fn tls_files(&self, app: &AppName) -> TlsFiles {
        let dir = self.cert_dir(app);
        TlsFiles {
            certificate: dir.join(CERTIFICATE_FILE),
            key: dir.join(KEY_FILE),
        }
    }
}

/// The key and certificate writes, or their removal when `tls` is `None`.
pub(crate) fn planned_pair(roots: &CaddyRoots, app: &AppName, tls: Option<&Tls>) -> [Planned; 2] {
    let files = roots.tls_files(app);
    [
        Planned {
            path: files.key,
            mode: SITE_FILE,
            contents: tls.map(|pair| pair.key_pem.clone().into_bytes()),
        },
        Planned {
            path: files.certificate,
            mode: SITE_FILE,
            contents: tls.map(|pair| pair.cert_pem.clone().into_bytes()),
        },
    ]
}

/// Create the certificate directories before a pair is written.
pub(crate) fn prepare_dirs(roots: &CaddyRoots, app: &AppName) -> Result<()> {
    [roots.cert_root(), roots.cert_dir(app)]
        .iter()
        .try_for_each(|dir| ensure_dir(dir, CERT_DIR_MODE, roots.owner_uid))
}

/// Remove the app's certificate directory once its files are gone. A
/// directory that is not empty or already missing is left alone.
pub(crate) fn remove_dir(roots: &CaddyRoots, app: &AppName) {
    let _ = fs::remove_dir(roots.cert_dir(app));
}
